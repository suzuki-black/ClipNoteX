use crate::{Formatter, FormatOptions, Language};
use clipnotex_core::Result;
use sqlformat::{format, FormatOptions as SqlOpts, Indent, QueryParams};

pub struct SqlFormatter;

impl Formatter for SqlFormatter {
    fn id(&self) -> &str {
        "sql"
    }

    fn languages(&self) -> &[Language] {
        &[Language::Sql]
    }

    fn format(&self, input: &str, opts: &FormatOptions) -> Result<String> {
        let indent_width = opts.indent.unwrap_or(2) as u8;
        let formatted = format(
            input,
            &QueryParams::None,
            &SqlOpts {
                indent: Indent::Spaces(indent_width),
                uppercase: Some(true),
                lines_between_queries: 1,
                ignore_case_convert: None,
            },
        );
        Ok(formatted)
    }
}
