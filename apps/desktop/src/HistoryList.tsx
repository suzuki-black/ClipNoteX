import { useEffect, useRef } from "react";
import type { ClipItemSummary } from "./api";
import { HistoryItem } from "./HistoryItem";

interface Props {
  items: ClipItemSummary[];
  selectedIndex: number;
  query: string;
  onSelect: (i: number) => void;
  onPaste: (id: string, mode: "normal" | "plain") => void;
  onFormatPaste: (id: string, preview: string) => void;
  onDelete: (id: string) => void;
}

export function HistoryList({
  items,
  selectedIndex,
  query,
  onSelect,
  onPaste,
  onFormatPaste,
  onDelete,
}: Props) {
  const listRef = useRef<HTMLUListElement>(null);

  // Scroll selected item into view when keyboard navigation changes selection.
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const el = list.querySelector<HTMLElement>(`[aria-selected="true"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  if (items.length === 0) {
    return (
      <div className="empty-state">
        {query ? `No results for "${query}"` : "No clipboard history yet"}
      </div>
    );
  }

  return (
    <ul
      ref={listRef}
      className="history-list"
      role="listbox"
      aria-label="Clipboard history"
    >
      {items.map((item, i) => (
        <HistoryItem
          key={item.id}
          item={item}
          isSelected={i === selectedIndex}
          query={query}
          onSelect={() => onSelect(i)}
          onPaste={(mode) => onPaste(item.id, mode)}
          onFormatPaste={() => onFormatPaste(item.id, item.preview)}
          onDelete={() => onDelete(item.id)}
        />
      ))}
    </ul>
  );
}
