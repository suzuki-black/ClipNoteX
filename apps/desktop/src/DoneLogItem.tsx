import { useState } from "react";
import type { DoneViewSummary, UpdateOverlayArgs } from "./api";
import { updateDoneOverlay } from "./api";

interface Props {
  item: DoneViewSummary;
  onUpdated: (updated: DoneViewSummary) => void;
}

export function DoneLogItem({ item, onUpdated }: Props) {
  const [editing, setEditing] = useState(false);
  const [noteInput, setNoteInput] = useState(item.note ?? "");
  const [tagInput, setTagInput] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function saveOverlay() {
    setSaving(true);
    setError(null);
    try {
      const args: UpdateOverlayArgs = {
        id: item.id,
        note: noteInput || undefined,
        add_tags: [],
        remove_tags: [],
      };
      await updateDoneOverlay(args);
      onUpdated({ ...item, note: noteInput || null });
      setEditing(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function addTag() {
    const tag = tagInput.trim().replace(/^#/, "");
    if (!tag || item.tags.includes(tag)) return;
    setSaving(true);
    setError(null);
    try {
      const args: UpdateOverlayArgs = {
        id: item.id,
        add_tags: [tag],
        remove_tags: [],
      };
      await updateDoneOverlay(args);
      onUpdated({ ...item, tags: [...item.tags, tag] });
      setTagInput("");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function removeTag(tag: string) {
    setSaving(true);
    setError(null);
    try {
      const args: UpdateOverlayArgs = {
        id: item.id,
        add_tags: [],
        remove_tags: [tag],
      };
      await updateDoneOverlay(args);
      onUpdated({ ...item, tags: item.tags.filter((t) => t !== tag) });
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <article className="done-item">
      <header className="done-item__header">
        <time className="done-item__time">{item.time}</time>
        <span className="done-item__app">{item.source_app}</span>
        <span className="done-item__kind">{item.kind}</span>
      </header>

      <p className="done-item__body">{item.body}</p>

      {/* Note section */}
      {editing ? (
        <div className="done-item__note-editor">
          <textarea
            className="done-item__note-input"
            value={noteInput}
            onChange={(e) => setNoteInput(e.target.value)}
            placeholder="メモを入力…"
            rows={2}
            autoFocus
          />
          <div className="done-item__note-actions">
            <button
              className="btn btn--primary btn--sm"
              onClick={saveOverlay}
              disabled={saving}
            >
              {saving ? "保存中…" : "保存"}
            </button>
            <button
              className="btn btn--sm"
              onClick={() => {
                setNoteInput(item.note ?? "");
                setEditing(false);
              }}
              disabled={saving}
            >
              キャンセル
            </button>
          </div>
        </div>
      ) : (
        <div className="done-item__note-row">
          {item.note ? (
            <p className="done-item__note">📝 {item.note}</p>
          ) : null}
          <button
            className="btn btn--ghost btn--sm done-item__edit-btn"
            onClick={() => setEditing(true)}
          >
            {item.note ? "編集" : "+ メモ"}
          </button>
        </div>
      )}

      {/* Tags */}
      <div className="done-item__tags">
        {item.tags.map((tag) => (
          <span key={tag} className="tag">
            #{tag}
            <button
              className="tag__remove"
              onClick={() => removeTag(tag)}
              disabled={saving}
              aria-label={`タグ #${tag} を削除`}
            >
              ✕
            </button>
          </span>
        ))}
        <form
          className="done-item__tag-form"
          onSubmit={(e) => {
            e.preventDefault();
            addTag();
          }}
        >
          <input
            className="done-item__tag-input"
            value={tagInput}
            onChange={(e) => setTagInput(e.target.value)}
            placeholder="#タグ"
            disabled={saving}
          />
          <button type="submit" className="btn btn--ghost btn--sm" disabled={saving || !tagInput.trim()}>
            追加
          </button>
        </form>
      </div>

      {error && <p className="done-item__error">{error}</p>}
    </article>
  );
}
