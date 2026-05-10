//! High-level orchestration of all the moving parts.

pub mod capture;
pub mod exclusion;
pub mod quota;
pub mod thumbnail;

pub use capture::run_capture_loop;
pub use exclusion::ExclusionFilter;
pub use quota::QuotaManager;
pub use thumbnail::ThumbnailService;
