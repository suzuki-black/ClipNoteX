use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use clipnotex_core::{CnxError, Result};
use zeroize::Zeroizing;

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

#[derive(Debug)]
pub enum KeySource {
    /// Pull from OS keychain (macOS Keychain / Windows Credential Manager).
    Keyring { service: String, account: String },
    /// In-memory random key — only for tests.
    Ephemeral,
}

pub struct DataKeys {
    pub history: Zeroizing<[u8; KEY_LEN]>,
    pub donelog: Zeroizing<[u8; KEY_LEN]>,
}

impl DataKeys {
    pub fn generate() -> Self {
        let mut h = [0u8; KEY_LEN];
        let mut d = [0u8; KEY_LEN];
        rand::Rng::fill(&mut rand::thread_rng(), &mut h);
        rand::Rng::fill(&mut rand::thread_rng(), &mut d);
        Self {
            history: Zeroizing::new(h),
            donelog: Zeroizing::new(d),
        }
    }

    pub fn load(source: &KeySource) -> Result<Self> {
        match source {
            KeySource::Ephemeral => Ok(Self::generate()),
            KeySource::Keyring { service, account } => {
                let entry = keyring::Entry::new(service, account)
                    .map_err(|e| CnxError::Crypto(format!("keyring open: {e}")))?;
                match entry.get_password() {
                    Ok(b64) => decode_keys(&b64),
                    Err(keyring::Error::NoEntry) => {
                        let keys = Self::generate();
                        entry
                            .set_password(&encode_keys(&keys))
                            .map_err(|e| CnxError::Crypto(format!("keyring write: {e}")))?;
                        Ok(keys)
                    }
                    Err(e) => Err(CnxError::Crypto(format!("keyring read: {e}"))),
                }
            }
        }
    }
}

fn encode_keys(keys: &DataKeys) -> String {
    let mut buf = [0u8; KEY_LEN * 2];
    buf[..KEY_LEN].copy_from_slice(&*keys.history);
    buf[KEY_LEN..].copy_from_slice(&*keys.donelog);
    use base64ish::encode;
    encode(&buf)
}

fn decode_keys(s: &str) -> Result<DataKeys> {
    use base64ish::decode;
    let raw = decode(s).map_err(|e| CnxError::Crypto(format!("decode keys: {e}")))?;
    if raw.len() != KEY_LEN * 2 {
        return Err(CnxError::Crypto("invalid key length".into()));
    }
    let mut h = [0u8; KEY_LEN];
    let mut d = [0u8; KEY_LEN];
    h.copy_from_slice(&raw[..KEY_LEN]);
    d.copy_from_slice(&raw[KEY_LEN..]);
    Ok(DataKeys {
        history: Zeroizing::new(h),
        donelog: Zeroizing::new(d),
    })
}

/// AAD-aware AEAD wrapper around a single 32-byte key.
pub struct Sealer {
    cipher: XChaCha20Poly1305,
}

impl Sealer {
    pub fn new(key: &[u8; KEY_LEN]) -> Self {
        Self {
            cipher: XChaCha20Poly1305::new(key.into()),
        }
    }

    pub fn seal(&self, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let payload = chacha20poly1305::aead::Payload {
            msg: plaintext,
            aad,
        };
        let ct = self
            .cipher
            .encrypt(&nonce, payload)
            .map_err(|e| CnxError::Crypto(format!("seal: {e}")))?;
        let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn open(&self, ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < NONCE_LEN {
            return Err(CnxError::Crypto("ciphertext too short".into()));
        }
        let (nonce_bytes, ct) = ciphertext.split_at(NONCE_LEN);
        let nonce = XNonce::from_slice(nonce_bytes);
        let payload = chacha20poly1305::aead::Payload { msg: ct, aad };
        self.cipher
            .decrypt(nonce, payload)
            .map_err(|e| CnxError::Crypto(format!("open: {e}")))
    }
}

/// Tiny URL-safe base64 helpers without pulling in the full base64 crate.
mod base64ish {
    const A: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    pub fn encode(data: &[u8]) -> String {
        let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
            let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(A[(n >> 18 & 0x3f) as usize] as char);
            out.push(A[(n >> 12 & 0x3f) as usize] as char);
            if chunk.len() > 1 {
                out.push(A[(n >> 6 & 0x3f) as usize] as char);
            }
            if chunk.len() > 2 {
                out.push(A[(n & 0x3f) as usize] as char);
            }
        }
        out
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, &'static str> {
        let mut idx = [255u8; 256];
        for (i, &b) in A.iter().enumerate() {
            idx[b as usize] = i as u8;
        }
        let bytes: Vec<u8> = s
            .bytes()
            .map(|b| idx[b as usize])
            .filter(|&i| i != 255)
            .collect();
        let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
        for chunk in bytes.chunks(4) {
            let b0 = chunk[0] as u32;
            let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
            let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
            let b3 = chunk.get(3).copied().unwrap_or(0) as u32;
            let n = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;
            out.push((n >> 16 & 0xff) as u8);
            if chunk.len() > 2 {
                out.push((n >> 8 & 0xff) as u8);
            }
            if chunk.len() > 3 {
                out.push((n & 0xff) as u8);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let keys = DataKeys::generate();
        let s = Sealer::new(&keys.history);
        let pt = b"hello world";
        let aad = b"format=text";
        let ct = s.seal(pt, aad).unwrap();
        let back = s.open(&ct, aad).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn aad_mismatch_fails() {
        let keys = DataKeys::generate();
        let s = Sealer::new(&keys.history);
        let ct = s.seal(b"data", b"a").unwrap();
        assert!(s.open(&ct, b"b").is_err());
    }
}
