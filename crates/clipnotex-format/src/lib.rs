//! Language detection and formatting for ClipNoteX.
//!
//! 提供フォーマッター：
//!  - JSON  — serde_json による pretty-print (M12)
//!  - SQL   — sqlformat crate による整形 (M12)
//!  - Markdown — 行末スペース正規化のみ (M12)
//!  - プレーンテキスト — タブ展開 / 行末トリム (M12)
//!
//! `Formatter` トレイトを実装した型を `FormatService` に登録して使う。
//! UI は `detect()` で言語を推定してから `FormatService::format_as()` を呼ぶ。

use clipnotex_core::{CnxError, Result};

mod json;
mod markdown;
mod plaintext;
mod sql;

pub use json::JsonFormatter;
pub use markdown::MarkdownFormatter;
pub use plaintext::PlainTextFormatter;
pub use sql::SqlFormatter;

// ---------------------------------------------------------------------------
// Language
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Language {
    Json,
    Sql,
    Markdown,
    Html,
    Css,
    JavaScript,
    TypeScript,
    Php,
    PlainText,
    Other(String),
}

// ---------------------------------------------------------------------------
// Formatter trait
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct FormatOptions {
    /// Indentation width in spaces (default: 2).
    pub indent: Option<u8>,
    /// Target line width (default: 120).
    pub line_width: Option<u16>,
    /// SQL dialect hint ("ansi" | "mysql" | "postgresql").
    pub dialect: Option<String>,
}

pub trait Formatter: Send + Sync {
    fn id(&self) -> &str;
    fn languages(&self) -> &[Language];
    fn format(&self, input: &str, opts: &FormatOptions) -> Result<String>;
}

// ---------------------------------------------------------------------------
// Dispatch service
// ---------------------------------------------------------------------------

pub struct FormatService {
    formatters: Vec<Box<dyn Formatter>>,
}

impl Default for FormatService {
    fn default() -> Self {
        Self {
            formatters: vec![
                Box::new(JsonFormatter),
                Box::new(SqlFormatter),
                Box::new(MarkdownFormatter),
                Box::new(PlainTextFormatter),
            ],
        }
    }
}

impl FormatService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Format `input` as `lang` using the matching formatter.
    /// Returns `Err` if no formatter supports the language.
    pub fn format_as(
        &self,
        lang: &Language,
        input: &str,
        opts: &FormatOptions,
    ) -> Result<String> {
        for f in &self.formatters {
            if f.languages().contains(lang) {
                return f.format(input, opts);
            }
        }
        Err(CnxError::Other(format!("no formatter for {lang:?}")))
    }
}

// ---------------------------------------------------------------------------
// Auto-detection
// ---------------------------------------------------------------------------

/// Heuristic language detection from clipboard text.
/// Returns `None` when no strong signal found (caller should leave unchanged).
pub fn detect(input: &str) -> Option<Language> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // JSON: starts with '{' or '[' and parses.
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    {
        return Some(Language::Json);
    }

    // SQL: leading keyword heuristic (case-insensitive).
    let upper = trimmed.to_uppercase();
    for keyword in &["SELECT ", "INSERT ", "UPDATE ", "DELETE ", "CREATE ", "DROP ", "ALTER "] {
        if upper.starts_with(keyword) {
            return Some(Language::Sql);
        }
    }

    // Markdown: has ATX headings or fenced code blocks.
    if trimmed.contains("\n# ")
        || trimmed.starts_with("# ")
        || trimmed.contains("\n```")
        || trimmed.starts_with("```")
    {
        return Some(Language::Markdown);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_pretty_prints() {
        let svc = FormatService::new();
        let input = r#"{"b":2,"a":1}"#;
        let out = svc.format_as(&Language::Json, input, &FormatOptions::default()).unwrap();
        assert!(out.contains('\n'));
        assert!(out.contains("  \"a\": 1"));
    }

    #[test]
    fn json_invalid_returns_error() {
        let svc = FormatService::new();
        let result = svc.format_as(&Language::Json, "not json", &FormatOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn json_custom_indent() {
        let svc = FormatService::new();
        let out = svc
            .format_as(
                &Language::Json,
                r#"{"a":1}"#,
                &FormatOptions { indent: Some(4), ..Default::default() },
            )
            .unwrap();
        assert!(out.contains("    \"a\": 1"));
    }

    #[test]
    fn sql_uppercases_keywords() {
        let svc = FormatService::new();
        let out = svc
            .format_as(&Language::Sql, "select id from users where id=1", &FormatOptions::default())
            .unwrap();
        assert!(out.contains("SELECT"), "got: {out}");
        assert!(out.contains("FROM"), "got: {out}");
    }

    #[test]
    fn markdown_trims_trailing_whitespace() {
        let svc = FormatService::new();
        let input = "# Heading   \n\nSome text  \n";
        let out = svc.format_as(&Language::Markdown, input, &FormatOptions::default()).unwrap();
        assert!(!out.contains("   "), "trailing spaces remain: {out:?}");
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn markdown_collapses_blank_lines() {
        let svc = FormatService::new();
        let input = "# A\n\n\n\nParagraph\n";
        let out = svc.format_as(&Language::Markdown, input, &FormatOptions::default()).unwrap();
        // Should not have more than one consecutive blank line.
        assert!(!out.contains("\n\n\n"), "excess blank lines: {out:?}");
    }

    #[test]
    fn detect_json() {
        assert_eq!(detect(r#"{"key": "value"}"#), Some(Language::Json));
        assert_eq!(detect(r#"[1, 2, 3]"#), Some(Language::Json));
    }

    #[test]
    fn detect_sql() {
        assert_eq!(detect("SELECT * FROM users"), Some(Language::Sql));
        assert_eq!(detect("  select id from t"), Some(Language::Sql));
    }

    #[test]
    fn detect_markdown() {
        assert_eq!(detect("# Heading\n\nContent"), Some(Language::Markdown));
    }

    #[test]
    fn detect_unknown_returns_none() {
        assert_eq!(detect("hello world"), None);
        assert_eq!(detect(""), None);
    }
}
