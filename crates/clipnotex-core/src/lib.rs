//! ClipNoteX core: shared types, settings, error and event bus.
//!
//! This crate has zero OS dependencies. It defines the vocabulary that the
//! rest of the workspace speaks.

pub mod bus;
pub mod error;
pub mod ids;
pub mod model;
pub mod settings;

pub use error::{CnxError, Result};
pub use ids::{ClipId, HotkeyId};
