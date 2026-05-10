/**
 * Tauri command bindings.
 * Backend: crates/clipnotex-tauri/src/commands.rs
 */
import { invoke } from "@tauri-apps/api/core";

export interface ClipItemSummary {
  id: string;
  created_at: number;
  kind: string;
  source_app: string;
  preview: string;
  pinned: boolean;
}

export interface PasteArgs {
  id: string;
  mode: "normal" | "plain" | "format";
  /** Only for mode="format": "auto" | "json" | "sql" | "markdown" | "plaintext" */
  format_lang?: string;
  /** Only for mode="format": indentation width in spaces */
  format_indent?: number;
}

export interface FormatPreviewResult {
  formatted: string;
  detected_lang: string;
}

export function listHistory(
  query?: string,
  limit?: number,
): Promise<ClipItemSummary[]> {
  return invoke("list_history", { query, limit });
}

export function pasteItem(args: PasteArgs): Promise<void> {
  return invoke("paste_item", { args });
}

export function pinToggle(id: string): Promise<boolean> {
  return invoke("pin_toggle", { id });
}

export function deleteItem(id: string): Promise<void> {
  return invoke("delete_item", { id });
}

export function enableInputFocus(): Promise<void> {
  return invoke("enable_input_focus");
}

// ---------------------------------------------------------------------------
// Format paste
// ---------------------------------------------------------------------------

/** Format text without pasting. Returns formatted text + detected language. */
export function formatPreview(
  text: string,
  lang?: string,
  indent?: number,
): Promise<FormatPreviewResult> {
  return invoke("format_preview", { text, lang, indent });
}

/** Auto-detect the language of a text snippet. Returns null if unknown. */
export function detectLang(text: string): Promise<string | null> {
  return invoke("detect_lang", { text });
}

// ---------------------------------------------------------------------------
// DONE LOG
// ---------------------------------------------------------------------------

export interface DoneViewSummary {
  id: string;
  date: string;   // "YYYY-MM-DD"
  time: string;   // "HH:MM"
  source_app: string;
  kind: string;
  body: string;
  note: string | null;
  tags: string[];
}

export interface UpdateOverlayArgs {
  id: string;
  note?: string;
  body?: string;
  add_tags: string[];
  remove_tags: string[];
}

export function captureDone(body: string): Promise<void> {
  return invoke("capture_done", { body });
}

export function listDone(options?: {
  date?: string;
  limit?: number;
}): Promise<DoneViewSummary[]> {
  return invoke("list_done", {
    date: options?.date,
    limit: options?.limit,
  });
}

export function updateDoneOverlay(args: UpdateOverlayArgs): Promise<void> {
  return invoke("update_done_overlay", { args });
}

export function exportDoneMarkdown(date?: string): Promise<string> {
  return invoke("export_done_markdown", { date });
}
