use crate::{Formatter, FormatOptions, Language};
use clipnotex_core::Result;

pub struct PlainTextFormatter;

impl Formatter for PlainTextFormatter {
    fn id(&self) -> &str {
        "plaintext"
    }

    fn languages(&self) -> &[Language] {
        &[Language::PlainText]
    }

    /// Normalise plain text:
    ///  - Expand tabs to spaces (default: 4).
    ///  - Trim trailing whitespace from each line.
    ///  - Ensure a single trailing newline.
    fn format(&self, input: &str, opts: &FormatOptions) -> Result<String> {
        let tab_width = opts.indent.unwrap_or(4) as usize;
        let tab_str = " ".repeat(tab_width);
        let mut lines: Vec<String> = input
            .lines()
            .map(|l| l.replace('\t', &tab_str).trim_end().to_string())
            .collect();
        while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
            lines.pop();
        }
        lines.push(String::new());
        Ok(lines.join("\n"))
    }
}
