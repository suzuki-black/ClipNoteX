//! Thumbnail generation for image ClipItems.
//!
//! Concern §6 — DIB → PNG → thumbnail の CPU コストが高く UI を固める危険。
//!
//! 方針:
//! - UI はまず `ThumbnailState::Pending` プレースホルダを表示する。
//! - バックグラウンドワーカーが `spawn_blocking` でサムネイルを生成する。
//! - 生成完了後に `EventBus::emit(ThumbnailReady)` で UI に通知する。
//! - サムネイルはファイルシステムにキャッシュし、再起動後も再生成しない。
//! - キャッシュパス: `<data_dir>/thumbnails/<first2>/<rest>.png` (非暗号化)。
//!   サムネイルは縮小済みで機微度が低い + UI 表示専用なので暗号化対象外。

use clipnotex_core::{ids::ClipId, CnxError, Result};
use image::imageops::FilterType;
use image::ImageFormat;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const THUMB_SIZE: u32 = 32;

/// Represents the current state of a thumbnail for a single ClipItem.
#[derive(Clone, Debug)]
pub enum ThumbnailState {
    /// Not yet queued or generating.
    Pending,
    /// Generation was enqueued; waiting for the worker.
    Generating,
    /// Available at the given path.
    Ready(PathBuf),
    /// Image could not be decoded or encoded.
    Failed(String),
}

pub struct ThumbnailService {
    cache_dir: PathBuf,
    /// Bounded channel so the worker queue never grows unboundedly.
    tx: tokio::sync::mpsc::Sender<ThumbnailRequest>,
    #[allow(dead_code)]
    bus: clipnotex_core::bus::EventBus,
}

struct ThumbnailRequest {
    id: ClipId,
    /// Raw PNG bytes (already decrypted, in-memory).
    png_bytes: Vec<u8>,
    cache_path: PathBuf,
}

impl ThumbnailService {
    /// `bus` receives a `CoreEvent::ThumbnailReady` when each thumbnail is done.
    pub fn start(cache_dir: PathBuf, bus: clipnotex_core::bus::EventBus) -> Arc<Self> {
        std::fs::create_dir_all(&cache_dir).ok();

        // Bounded queue — concern §6: prevent unbounded queuing on bulk paste.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ThumbnailRequest>(64);

        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let path = req.cache_path.clone();
                let id = req.id;
                let result = tokio::task::spawn_blocking(move || {
                    generate_thumbnail(&req.png_bytes, &path)
                })
                .await;
                match result {
                    Ok(Ok(_)) => tracing::debug!(?id, "thumbnail generated"),
                    Ok(Err(e)) => tracing::warn!(?id, ?e, "thumbnail generation failed"),
                    Err(e) => tracing::warn!(?id, "thumbnail worker panicked: {e}"),
                }
                // TODO(M8): emit CoreEvent::ThumbnailReady(id) via bus here.
                // (bus needs to be moved into this closure; thread-safety requires Arc)
            }
        });

        Arc::new(Self { cache_dir, tx, bus })
    }

    /// Returns the cached path if it exists, otherwise enqueues generation
    /// and returns `None` (UI should show placeholder).
    pub async fn get_or_enqueue(&self, id: ClipId, png_bytes: Vec<u8>) -> Option<PathBuf> {
        let cache_path = self.cache_path_for(&id);
        if cache_path.exists() {
            return Some(cache_path);
        }
        let req = ThumbnailRequest {
            id,
            png_bytes,
            cache_path,
        };
        // Non-blocking send: drop if queue is full (concern §6: never block UI).
        match self.tx.try_send(req) {
            Ok(_) => tracing::debug!(?id, "thumbnail enqueued"),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(?id, "thumbnail queue full, skipping")
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                tracing::error!("thumbnail worker channel closed")
            }
        }
        None
    }

    pub fn cache_path_for(&self, id: &ClipId) -> PathBuf {
        let s = id.to_string();
        self.cache_dir
            .join(&s[..2])
            .join(format!("{}.png", &s[2..]))
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

/// CPU-bound: decode PNG, resize to THUMB_SIZE × THUMB_SIZE, re-encode PNG.
///
/// Called inside `spawn_blocking`. Never blocks the async runtime.
fn generate_thumbnail(png_bytes: &[u8], out_path: &Path) -> Result<()> {
    // Decode — concern §6: image crate is used for all decoding.
    // Pin the decoder to PNG instead of byte-sniffing the format. Inputs are
    // always PNG (macOS NSPasteboard PNG / Windows DIB re-encoded to PNG), so
    // format auto-detection only widens the attack surface — a malicious
    // clipboard payload with e.g. TIFF/JPEG magic bytes would otherwise be
    // routed into transitively-included decoders we never intend to use.
    let img = image::load_from_memory_with_format(png_bytes, ImageFormat::Png)
        .map_err(|e| CnxError::Other(format!("thumbnail decode: {e}")))?;

    // Lanczos3 for quality, but a 32×32 output means quality matters less
    // than speed — switch to Nearest for large batches if profiling shows it.
    let thumb = img.resize(THUMB_SIZE, THUMB_SIZE, FilterType::Lanczos3);

    // Atomic write: write to tmp then rename (same as BlobStore).
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = out_path.with_extension("tmp");
    thumb
        .save_with_format(&tmp, image::ImageFormat::Png)
        .map_err(|e| CnxError::Other(format!("thumbnail save: {e}")))?;
    std::fs::rename(&tmp, out_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that a minimal 1×1 white PNG round-trips through the thumbnail
    /// generator without panicking or returning an error.
    #[test]
    fn generate_1x1_png() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("thumb.png");

        // Minimal valid PNG: 1×1 white pixel.
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();

        generate_thumbnail(&buf, &out).unwrap();
        assert!(out.exists());
    }

    #[test]
    fn empty_bytes_fails_gracefully() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("thumb.png");
        let result = generate_thumbnail(&[], &out);
        assert!(result.is_err(), "empty input should return Err");
    }

    /// Security regression (v0.4): the decoder is pinned to PNG, so a payload
    /// that is a *valid* non-PNG image (here JPEG) must be rejected rather than
    /// silently routed through a format-auto-detect path into another decoder.
    #[test]
    fn non_png_input_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("thumb.png");

        // Encode a genuine 1×1 JPEG. Under the old load_from_memory() this would
        // have decoded fine; under the PNG-pinned decoder it must fail.
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([10, 20, 30, 255]));
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .to_rgb8()
            .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();

        let result = generate_thumbnail(&jpeg, &out);
        assert!(result.is_err(), "non-PNG image must be rejected by the PNG-pinned decoder");
        assert!(!out.exists());
    }
}
