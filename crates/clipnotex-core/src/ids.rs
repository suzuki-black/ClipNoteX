use serde::{Deserialize, Serialize};
use ulid::Ulid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClipId(pub Ulid);

impl ClipId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
    pub fn as_bytes(&self) -> [u8; 16] {
        self.0.to_bytes()
    }
}

impl Default for ClipId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ClipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum HotkeyId {
    ShowHistory,
    ShowSnippets,
    PastePlain,
    PasteFormat,
    PasteFull,
    DoneCapture,
}
