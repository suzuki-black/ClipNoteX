//! Hot blob store. Pack-file consolidation comes in v0.2 (DESIGN §3.5).

use clipnotex_core::{model::BlobId, CnxError, Result};
use std::path::{Path, PathBuf};

pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn path(&self, id: &BlobId) -> PathBuf {
        let hex = hex_two(&id.0);
        self.root.join(&hex[..2]).join(&hex[2..])
    }

    pub fn write(&self, id: &BlobId, ciphertext: &[u8]) -> Result<()> {
        let path = self.path(id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Atomic write: tmp + rename.
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, ciphertext)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn read(&self, id: &BlobId) -> Result<Vec<u8>> {
        std::fs::read(self.path(id)).map_err(CnxError::Io)
    }

    pub fn delete(&self, id: &BlobId) -> Result<()> {
        let p = self.path(id);
        if p.exists() {
            std::fs::remove_file(p)?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn hex_two(b: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}
