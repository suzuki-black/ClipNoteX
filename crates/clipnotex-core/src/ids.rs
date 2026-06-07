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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_id_is_unique() {
        let a = ClipId::new();
        let b = ClipId::new();
        assert_ne!(a, b, "freshly generated ClipIds must differ");
    }

    #[test]
    fn clip_id_as_bytes_is_16() {
        assert_eq!(ClipId::new().as_bytes().len(), 16);
    }

    #[test]
    fn clip_id_serde_is_transparent_string() {
        // #[serde(transparent)] => serializes exactly like the inner Ulid string.
        let id = ClipId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        let back: ClipId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn hotkey_id_ordering_matches_declaration() {
        // derive(Ord) orders by variant declaration order; relied on by the
        // BTreeMap<HotkeyId, _> in Settings.
        assert!(HotkeyId::ShowHistory < HotkeyId::ShowSnippets);
        assert!(HotkeyId::ShowSnippets < HotkeyId::DoneCapture);
    }

    #[test]
    fn hotkey_id_serde_roundtrip() {
        for hk in [
            HotkeyId::ShowHistory,
            HotkeyId::PastePlain,
            HotkeyId::DoneCapture,
        ] {
            let json = serde_json::to_string(&hk).unwrap();
            let back: HotkeyId = serde_json::from_str(&json).unwrap();
            assert_eq!(hk, back);
        }
    }
}
