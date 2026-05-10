use clipnotex_core::model::SourceApp;

/// Best-effort detection of the application that produced the most
/// recent clipboard write.
///
/// Resolution order (DESIGN §4.2):
///   1. `org.nspasteboard.source` (mac) / clipboard owner process (win)
///   2. Frontmost / foreground application
///   3. Window title only (last resort)
pub fn current() -> SourceApp {
    #[cfg(target_os = "macos")]
    {
        super::macos::detect_source().unwrap_or_default()
    }
    #[cfg(target_os = "windows")]
    {
        super::windows::detect_source().unwrap_or_default()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        SourceApp::default()
    }
}
