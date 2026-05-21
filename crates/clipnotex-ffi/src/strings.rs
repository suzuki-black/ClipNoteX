//! C string interop helpers.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Convert a Rust `String` into a heap-allocated C string.
/// Caller MUST free via [`crate::api::cnx_free_string`].
pub(crate) fn into_c_string(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Borrow a C string as `&str`. Returns `None` if `ptr` is null or invalid UTF-8.
pub(crate) unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}
