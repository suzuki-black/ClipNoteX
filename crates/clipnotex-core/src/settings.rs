use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::ids::HotkeyId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    pub history: HistoryConfig,
    pub shortcuts: BTreeMap<HotkeyId, ShortcutBinding>,
    pub exclusions: Vec<ExclusionRule>,
    pub respect_concealed_pasteboard: bool,
    pub self_write_ignore_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            history: HistoryConfig::default(),
            shortcuts: default_shortcuts(),
            exclusions: default_exclusions(),
            respect_concealed_pasteboard: true,
            self_write_ignore_ms: 800,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryConfig {
    pub max_items: u64,
    pub max_bytes: u64,
    pub eviction_policy: EvictionPolicy,
    pub keep_pinned: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_items: 1000,
            max_bytes: 500 * 1024 * 1024,
            eviction_policy: EvictionPolicy::SizePriority,
            keep_pinned: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EvictionPolicy {
    CountPriority,
    SizePriority,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShortcutBinding {
    pub macos: Option<String>,
    pub windows: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "match", content = "value", rename_all = "snake_case")]
pub enum ExclusionRule {
    BundleId(String),
    ExeBasename {
        name: String,
        #[serde(default)]
        fuzzy: bool,
    },
    /// Glob over window title (e.g. "*1Password*").
    WindowTitle(String),
}

fn default_shortcuts() -> BTreeMap<HotkeyId, ShortcutBinding> {
    use HotkeyId::*;
    let mut m = BTreeMap::new();
    m.insert(ShowHistory, b("Cmd+Shift+V", "Ctrl+Shift+V"));
    m.insert(ShowSnippets, b("Cmd+Shift+C", "Ctrl+Shift+C"));
    m.insert(PastePlain, b("Cmd+Ctrl+V", "Ctrl+Shift+Alt+V"));
    m.insert(PasteFormat, b("Cmd+Alt+V", "Ctrl+Alt+V"));
    m.insert(PasteFull, b("Cmd+Shift+Alt+V", "Ctrl+Shift+Alt+F"));
    m.insert(DoneCapture, b("Cmd+Shift+D", "Ctrl+Shift+D"));
    m
}

fn b(mac: &str, win: &str) -> ShortcutBinding {
    ShortcutBinding {
        macos: Some(mac.into()),
        windows: Some(win.into()),
    }
}

fn default_exclusions() -> Vec<ExclusionRule> {
    use ExclusionRule::*;
    vec![
        BundleId("com.1password.1password".into()),
        BundleId("com.1password.1password7".into()),
        BundleId("com.bitwarden.desktop".into()),
        BundleId("org.keepassxc.keepassxc".into()),
        ExeBasename {
            name: "1Password".into(),
            fuzzy: true,
        },
        ExeBasename {
            name: "Bitwarden".into(),
            fuzzy: true,
        },
        ExeBasename {
            name: "KeePassXC".into(),
            fuzzy: true,
        },
    ]
}
