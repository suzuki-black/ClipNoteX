use crate::{Formatter, FormatOptions, Language};
use clipnotex_core::{CnxError, Result};

pub struct JsonFormatter;

impl Formatter for JsonFormatter {
    fn id(&self) -> &str {
        "json"
    }

    fn languages(&self) -> &[Language] {
        &[Language::Json]
    }

    fn format(&self, input: &str, opts: &FormatOptions) -> Result<String> {
        let indent = opts.indent.unwrap_or(2);

        let value: serde_json::Value = serde_json::from_str(input.trim())
            .map_err(|e| CnxError::Other(format!("JSON parse error: {e}")))?;

        // serde_json only supports 2-space indent natively; for other widths
        // we build an indented string and then re-indent.
        let pretty = serde_json::to_string_pretty(&value)
            .map_err(|e| CnxError::Other(format!("JSON serialize error: {e}")))?;

        if indent == 2 {
            return Ok(pretty);
        }

        // Re-indent from 2-space to requested width.
        let target = " ".repeat(indent as usize);
        let reindented = pretty
            .lines()
            .map(|line| {
                let leading = line.len() - line.trim_start_matches(' ').len();
                let levels = leading / 2;
                format!("{}{}", target.repeat(levels), line.trim_start())
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(reindented)
    }
}
