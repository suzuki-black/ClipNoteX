import type { ClipItemSummary } from "./api";

interface Props {
  item: ClipItemSummary;
  isSelected: boolean;
  query: string;
  onSelect: () => void;
  onPaste: (mode: "normal" | "plain") => void;
  onFormatPaste: () => void;
  onDelete: () => void;
}

const KIND_ICON: Record<string, string> = {
  Text: "📄",
  Rtf: "📝",
  Html: "🌐",
  Image: "🖼",
  Pdf: "📑",
  Files: "📁",
  Custom: "📦",
};

function highlight(text: string, query: string): React.ReactNode {
  if (!query) return text;
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx === -1) return text;
  return (
    <>
      {text.slice(0, idx)}
      <mark>{text.slice(idx, idx + query.length)}</mark>
      {text.slice(idx + query.length)}
    </>
  );
}

function relativeTime(ms: number): string {
  const diff = Date.now() - ms;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return new Date(ms).toLocaleDateString();
}

export function HistoryItem({
  item,
  isSelected,
  query,
  onSelect,
  onPaste,
  onFormatPaste,
  onDelete,
}: Props) {
  const icon = KIND_ICON[item.kind] ?? "📦";

  return (
    <li
      className={`history-item${isSelected ? " selected" : ""}`}
      onClick={onSelect}
      onDoubleClick={() => onPaste("normal")}
      role="option"
      aria-selected={isSelected}
    >
      <span className="item-icon" aria-label={item.kind}>{icon}</span>

      <div className="item-body">
        <p className="item-preview">
          {highlight(item.preview || `(${item.kind})`, query)}
        </p>
        <div className="item-meta">
          <span className="item-app">{item.source_app || "Unknown"}</span>
          <span className="item-time">{relativeTime(item.created_at)}</span>
        </div>
      </div>

      {isSelected && (
        <div className="item-actions">
          <button
            className="action-btn paste"
            onClick={(e) => { e.stopPropagation(); onPaste("normal"); }}
            title="Paste (Enter)"
          >
            Paste
          </button>
          <button
            className="action-btn plain"
            onClick={(e) => { e.stopPropagation(); onPaste("plain"); }}
            title="Paste as plain text (Shift+Enter)"
          >
            Plain
          </button>
          <button
            className="action-btn format"
            onClick={(e) => { e.stopPropagation(); onFormatPaste(); }}
            title="Format & Paste (Alt+Enter)"
          >
            ✨
          </button>
          <button
            className="action-btn delete"
            onClick={(e) => { e.stopPropagation(); onDelete(); }}
            title="Delete"
            aria-label="Delete item"
          >
            🗑
          </button>
        </div>
      )}
    </li>
  );
}
