use crate::{Formatter, FormatOptions, Language};
use clipnotex_core::Result;

pub struct MarkdownFormatter;

impl Formatter for MarkdownFormatter {
    fn id(&self) -> &str {
        "markdown"
    }

    fn languages(&self) -> &[Language] {
        &[Language::Markdown]
    }

    /// Normalise Markdown:
    ///  - Trim trailing whitespace from each line (except fenced code blocks).
    ///  - Ensure exactly one blank line between block elements.
    ///  - Ensure a single trailing newline.
    fn format(&self, input: &str, _opts: &FormatOptions) -> Result<String> {
        let mut in_fence = false;
        let mut lines: Vec<String> = input
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("```") {
                    in_fence = !in_fence;
                }
                if in_fence {
                    line.to_string()
                } else {
                    line.trim_end().to_string()
                }
            })
            .collect();

        // Remove consecutive blank lines (collapse to one).
        let mut result: Vec<String> = Vec::with_capacity(lines.len());
        let mut prev_blank = false;
        for line in lines.drain(..) {
            let blank = line.trim().is_empty();
            if blank && prev_blank {
                continue; // skip second consecutive blank
            }
            prev_blank = blank;
            result.push(line);
        }

        // Ensure single trailing newline.
        while result.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
            result.pop();
        }
        result.push(String::new());

        Ok(result.join("\n"))
    }
}
