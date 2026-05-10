use thiserror::Error;

#[derive(Debug, Error)]
pub enum CnxError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialize: {0}")]
    Serialize(String),

    #[error("storage: {0}")]
    Storage(String),

    #[error("crypto: {0}")]
    Crypto(String),

    #[error("clipboard: {0}")]
    Clipboard(String),

    #[error("hotkey: {0}")]
    Hotkey(String),

    #[error("paste: {0}")]
    Paste(String),

    #[error("not found")]
    NotFound,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CnxError>;
