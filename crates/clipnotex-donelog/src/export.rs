//! Export DONE LOG entries as Markdown daily digest or JSON.
//!
//! ## Markdown format (daily digest)
//!
//! ```markdown
//! # DONE LOG — 2026-05-10
//!
//! ## 09:42 — TestApp
//!
//! hello world
//!
//! > 📝 important note
//!
//! `#work` `#rust`
//!
//! ---
//! ```

use crate::DoneView;
use clipnotex_core::{CnxError, Result};

// ---------------------------------------------------------------------------
// Markdown export
// ---------------------------------------------------------------------------

/// Render a list of `DoneView`s as a Markdown daily digest string.
/// `views` should already be sorted and filtered to one day by the caller.
pub fn to_markdown(date: chrono::NaiveDate, views: &[DoneView]) -> String {
    let mut out = String::new();

    out.push_str(&format!("# DONE LOG — {}\n", date.format("%Y-%m-%d")));

    if views.is_empty() {
        out.push_str("\n_（記録なし）_\n");
        return out;
    }

    for view in views {
        out.push('\n');
        // Heading: time — app name
        out.push_str(&format!(
            "## {} — {}\n\n",
            view.entry.time.format("%H:%M"),
            view.entry.source_app.display_name,
        ));

        // Body (user override or original).
        let body = view.effective_body().trim();
        if !body.is_empty() {
            out.push_str(body);
            out.push_str("\n\n");
        }

        // Note.
        if let Some(note) = view.note() {
            let note = note.trim();
            if !note.is_empty() {
                out.push_str(&format!("> 📝 {note}\n\n"));
            }
        }

        // Tags.
        let tags = view.tags();
        if !tags.is_empty() {
            let tag_str: Vec<String> = tags.iter().map(|t| format!("`#{t}`")).collect();
            out.push_str(&tag_str.join(" "));
            out.push_str("\n\n");
        }

        out.push_str("---\n");
    }

    out
}

// ---------------------------------------------------------------------------
// JSON export
// ---------------------------------------------------------------------------

/// Serialize a list of `DoneView`s to a pretty-printed JSON string.
pub fn to_json(views: &[DoneView]) -> Result<String> {
    serde_json::to_string_pretty(views)
        .map_err(|e| CnxError::Other(format!("donelog JSON export: {e}")))
}

// ---------------------------------------------------------------------------
// File export helpers
// ---------------------------------------------------------------------------

/// Write a Markdown daily digest to `path`.
pub fn write_markdown(path: &std::path::Path, date: chrono::NaiveDate, views: &[DoneView]) -> Result<()> {
    let content = to_markdown(date, views);
    std::fs::write(path, content)?;
    Ok(())
}

/// Write a JSON export to `path`.
pub fn write_json(path: &std::path::Path, views: &[DoneView]) -> Result<()> {
    let content = to_json(views)?;
    std::fs::write(path, content)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentKind, DoneEntry, DoneOverlay, DoneView};
    use clipnotex_core::{ids::ClipId, model::SourceApp};
    use chrono::NaiveDate;
    use ulid::Ulid;

    fn make_view(body: &str, note: Option<&str>, tags: &[&str]) -> DoneView {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let entry = DoneEntry::new(
            ClipId(Ulid::new()),
            now_ms,
            SourceApp {
                bundle_id: None,
                exe_basename: None,
                exe_path: None,
                display_name: "TestApp".into(),
                window_title: None,
            },
            ContentKind::Text,
            body.to_string(),
            None,
        );
        let mut overlay = DoneOverlay::default();
        if let Some(n) = note {
            overlay.set_note(n);
        }
        for t in tags {
            overlay.add_tag(*t);
        }
        DoneView::new(entry, overlay)
    }

    #[test]
    fn markdown_contains_body_and_tag() {
        let views = vec![make_view("finished the report", Some("great work"), &["work"])];
        let date = NaiveDate::from_ymd_opt(2026, 5, 10).unwrap();
        let md = to_markdown(date, &views);
        assert!(md.contains("# DONE LOG — 2026-05-10"), "missing heading: {md}");
        assert!(md.contains("finished the report"), "missing body: {md}");
        assert!(md.contains("> 📝 great work"), "missing note: {md}");
        assert!(md.contains("`#work`"), "missing tag: {md}");
    }

    #[test]
    fn markdown_empty_day() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 10).unwrap();
        let md = to_markdown(date, &[]);
        assert!(md.contains("記録なし"));
    }

    #[test]
    fn json_round_trips() {
        let views = vec![make_view("test entry", None, &["rust"])];
        let json = to_json(&views).unwrap();
        let parsed: Vec<DoneView> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0].entry.body, "test entry");
        assert_eq!(parsed[0].tags(), &["rust"]);
    }
}
