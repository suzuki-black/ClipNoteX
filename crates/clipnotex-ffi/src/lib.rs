//! ClipNoteX FFI — C ABI surface for Swift / AppKit (macOS) and other native frontends.
//!
//! # Design
//! - Heavy / complex values are serialized to JSON; simple values use native C types
//! - All string returns are `*mut c_char` allocated by Rust; caller MUST free via
//!   [`cnx_free_string`].
//! - Errors are returned as a non-zero status code; details retrievable via
//!   [`cnx_last_error`] (thread-local).
//! - Tokio runtime is set up once in [`cnx_init`] and reused for all async work.
//!
//! # Memory rules
//! ```text
//! Rust → C : Box::leak + caller frees via cnx_free_string
//! C → Rust : caller retains ownership; Rust copies on entry
//! ```

#![allow(clippy::missing_safety_doc)] // documented in C header

mod errors;
mod runtime;
mod state;
mod strings;

pub mod api;

pub use api::*;
