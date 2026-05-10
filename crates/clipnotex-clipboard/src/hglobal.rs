//! Safe HGLOBAL accessor for the Windows OLE clipboard.
//!
//! All `unsafe` in this crate is confined here (DESIGN §5.2, concern §2).
//! Every public function returns a `Result`; callers never see raw pointers.
//!
//! # Safety contract
//! - Callers must call every function while the clipboard is open
//!   (`OpenClipboard` has been called, `CloseClipboard` has not).
//! - Each returned `Vec<u8>` is an owned copy — the HGLOBAL is unlocked
//!   and left intact before this module returns.

#[cfg(target_os = "windows")]
pub use win::*;

#[cfg(target_os = "windows")]
mod win {
    use clipnotex_core::{CnxError, Result};
    use windows::Win32::System::{
        DataExchange::GetClipboardData,
        Memory::{GlobalLock, GlobalSize, GlobalUnlock, HGLOBAL},
    };

    /// Copy the raw bytes of an HGLOBAL clipboard handle into a `Vec<u8>`.
    ///
    /// Returns `Err` on any of:
    /// - `GetClipboardData` returns NULL (format not available)
    /// - `GlobalLock` returns NULL (handle already destroyed / damaged)
    /// - `GlobalSize` returns 0 (empty or corrupt handle)
    pub fn copy_format(format: u32) -> Result<Vec<u8>> {
        // SAFETY: clipboard is open by the caller's contract.
        let handle = unsafe { GetClipboardData(format) }
            .map_err(|e| CnxError::Clipboard(format!("GetClipboardData({}): {}", format, e)))?;

        if handle.is_invalid() {
            return Err(CnxError::Clipboard(format!(
                "GetClipboardData({format}) returned invalid handle"
            )));
        }

        // SAFETY: handle came from GetClipboardData and is non-null.
        let hglobal: HGLOBAL = HGLOBAL(handle.0);
        copy_hglobal(hglobal, format)
    }

    /// Internal: lock + copy + unlock an HGLOBAL.
    fn copy_hglobal(hglobal: HGLOBAL, format_hint: u32) -> Result<Vec<u8>> {
        // SAFETY: hglobal is non-null and was obtained from the clipboard.
        let size = unsafe { GlobalSize(hglobal) };
        if size == 0 {
            return Err(CnxError::Clipboard(format!(
                "GlobalSize is 0 for format {format_hint} — handle may be empty or corrupt"
            )));
        }

        // SAFETY: hglobal is valid and size > 0.
        let ptr = unsafe { GlobalLock(hglobal) };
        if ptr.is_null() {
            return Err(CnxError::Clipboard(format!(
                "GlobalLock returned NULL for format {format_hint}"
            )));
        }

        // Always unlock, even on early return.
        struct Guard(HGLOBAL);
        impl Drop for Guard {
            fn drop(&mut self) {
                // SAFETY: locked by GlobalLock above; must be unlocked.
                unsafe { GlobalUnlock(self.0) };
            }
        }
        let _guard = Guard(hglobal);

        // SAFETY: ptr is valid for `size` bytes while the guard is alive.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, size) }.to_vec();
        Ok(bytes)
    }

    /// Parse `CF_DIB` / `CF_DIBV5` bytes into a PNG-encoded `Vec<u8>`.
    ///
    /// The `biSize` field in `BITMAPINFOHEADER` is trusted to determine
    /// the header variant (concern §2.1). Image decoding is delegated to
    /// the `image` crate to avoid handrolling pixel math.
    ///
    /// Returns `Err` when:
    /// - fewer than 40 bytes in the buffer (can't hold even a minimal header)
    /// - `biSize` indicates a header larger than the total buffer
    /// - the `image` crate cannot decode the DIB
    pub fn dib_to_png(raw: &[u8]) -> Result<Vec<u8>> {
        // BITMAPINFOHEADER minimum size is 40 bytes.
        if raw.len() < 40 {
            return Err(CnxError::Clipboard(format!(
                "CF_DIB too short ({} bytes, need ≥ 40)",
                raw.len()
            )));
        }

        // biSize is at offset 0, little-endian u32 (concern §2 — trust biSize, not magic).
        let bi_size = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
        if bi_size > raw.len() {
            return Err(CnxError::Clipboard(format!(
                "BITMAPINFOHEADER biSize ({bi_size}) > buffer length ({})",
                raw.len()
            )));
        }

        // Check for CF_DIBV5 color masks corruption heuristic (concern §2.1):
        // BITMAPV5HEADER is 124 bytes. If biSize == 124 but the buffer has a
        // suspicious color-mask region (all-zero masks for RGB), log a warning
        // and fall through — image crate will reject it if truly corrupt.
        if bi_size == 124 && raw.len() >= 124 {
            let r_mask = u32::from_le_bytes(raw[40..44].try_into().unwrap());
            let g_mask = u32::from_le_bytes(raw[44..48].try_into().unwrap());
            let b_mask = u32::from_le_bytes(raw[48..52].try_into().unwrap());
            if r_mask == 0 && g_mask == 0 && b_mask == 0 {
                tracing::warn!(
                    "CF_DIBV5 has all-zero color masks — may be corrupt, \
                     attempting decode anyway"
                );
            }
        }

        // Prepend a BITMAPFILEHEADER so image crate can recognize the format.
        let file_size = (14u32).saturating_add(raw.len() as u32);
        let pixel_offset = 14u32 + bi_size as u32;
        let mut bmp = Vec::with_capacity(14 + raw.len());
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&file_size.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes()); // reserved
        bmp.extend_from_slice(&pixel_offset.to_le_bytes());
        bmp.extend_from_slice(raw);

        // image crate handles the actual decoding (concern §2 — delegate to it).
        let img = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp)
            .map_err(|e| CnxError::Clipboard(format!("DIB decode: {e}")))?;

        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .map_err(|e| CnxError::Clipboard(format!("PNG encode: {e}")))?;
        Ok(out)
    }

    /// Extract a file path list from `CF_HDROP` bytes.
    ///
    /// Paths are returned as `Vec<String>` (UTF-8). The files themselves are
    /// NOT read or copied — see DESIGN §10.2 for the rationale.
    pub fn hdrop_paths(raw: &[u8]) -> Result<Vec<String>> {
        use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
        use windows::core::PWSTR;

        if raw.len() < 20 {
            return Err(CnxError::Clipboard("CF_HDROP buffer too short".into()));
        }

        // The DROPFILES struct is at the start of the HGLOBAL; we create a
        // temporary HDROP from the raw pointer for DragQueryFileW.
        // SAFETY: raw is owned, lives for this scope, HDROP only aliases it.
        let hdrop: HDROP = HDROP(raw.as_ptr() as isize);
        let count = unsafe { DragQueryFileW(hdrop, 0xFFFF_FFFF, None) };
        let mut paths = Vec::with_capacity(count as usize);
        for i in 0..count {
            let len = unsafe { DragQueryFileW(hdrop, i, None) } as usize;
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; len + 1];
            let filled =
                unsafe { DragQueryFileW(hdrop, i, Some(PWSTR(buf.as_mut_ptr())), len as u32 + 1) };
            if filled > 0 {
                let s = String::from_utf16_lossy(&buf[..filled as usize]);
                paths.push(s);
            }
        }
        Ok(paths)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn dib_too_short_rejected() {
            assert!(dib_to_png(&[0u8; 39]).is_err());
        }

        #[test]
        fn dib_bisize_overflow_rejected() {
            let mut raw = vec![0u8; 40];
            // Set biSize = 9999 which exceeds buffer.
            raw[0..4].copy_from_slice(&9999u32.to_le_bytes());
            assert!(dib_to_png(&raw).is_err());
        }
    }
}
