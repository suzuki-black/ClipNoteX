import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import {
  deleteItem,
  listHistory,
  pasteItem,
  type ClipItemSummary,
} from "./api";
import { DoneLog } from "./DoneLog";
import { FormatPasteModal } from "./FormatPasteModal";
import { HistoryList } from "./HistoryList";
import { SearchBar } from "./SearchBar";

// ---------------------------------------------------------------------------
// Clipboard history state
// ---------------------------------------------------------------------------

interface HistoryState {
  items: ClipItemSummary[];
  query: string;
  selectedIndex: number;
  loading: boolean;
  error: string | null;
}

type HistoryAction =
  | { type: "SET_ITEMS"; items: ClipItemSummary[] }
  | { type: "SET_QUERY"; query: string }
  | { type: "SET_SELECTED"; index: number }
  | { type: "SET_LOADING"; loading: boolean }
  | { type: "SET_ERROR"; error: string | null }
  | { type: "REMOVE_ITEM"; id: string };

function historyReducer(
  state: HistoryState,
  action: HistoryAction,
): HistoryState {
  switch (action.type) {
    case "SET_ITEMS":
      return { ...state, items: action.items, selectedIndex: 0, error: null };
    case "SET_QUERY":
      return { ...state, query: action.query, selectedIndex: 0 };
    case "SET_SELECTED":
      return { ...state, selectedIndex: action.index };
    case "SET_LOADING":
      return { ...state, loading: action.loading };
    case "SET_ERROR":
      return { ...state, error: action.error };
    case "REMOVE_ITEM": {
      const items = state.items.filter((i) => i.id !== action.id);
      const selectedIndex = Math.min(
        state.selectedIndex,
        items.length - 1,
      );
      return {
        ...state,
        items,
        selectedIndex: Math.max(0, selectedIndex),
      };
    }
  }
}

const HISTORY_INITIAL: HistoryState = {
  items: [],
  query: "",
  selectedIndex: 0,
  loading: true,
  error: null,
};

// ---------------------------------------------------------------------------
// History tab
// ---------------------------------------------------------------------------

interface FormatTarget {
  id: string;
  preview: string;
}

function HistoryTab() {
  const [state, dispatch] = useReducer(historyReducer, HISTORY_INITIAL);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [formatTarget, setFormatTarget] = useState<FormatTarget | null>(null);

  const load = useCallback((query: string) => {
    dispatch({ type: "SET_LOADING", loading: true });
    listHistory(query || undefined, 50)
      .then((items) => dispatch({ type: "SET_ITEMS", items }))
      .catch((e: unknown) =>
        dispatch({ type: "SET_ERROR", error: String(e) }),
      )
      .finally(() => dispatch({ type: "SET_LOADING", loading: false }));
  }, []);

  useEffect(() => {
    load("");
  }, [load]);

  function handleQueryChange(query: string) {
    dispatch({ type: "SET_QUERY", query });
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => load(query), 200);
  }

  async function handlePaste(id: string, mode: "normal" | "plain") {
    try {
      await pasteItem({ id, mode });
    } catch (e) {
      dispatch({ type: "SET_ERROR", error: String(e) });
    }
  }

  function handleFormatPaste(id: string, preview: string) {
    setFormatTarget({ id, preview });
  }

  async function handleDelete(id: string) {
    try {
      await deleteItem(id);
      dispatch({ type: "REMOVE_ITEM", id });
    } catch (e) {
      dispatch({ type: "SET_ERROR", error: String(e) });
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    const { items, selectedIndex } = state;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        dispatch({
          type: "SET_SELECTED",
          index: Math.min(selectedIndex + 1, items.length - 1),
        });
        break;
      case "ArrowUp":
        e.preventDefault();
        dispatch({
          type: "SET_SELECTED",
          index: Math.max(selectedIndex - 1, 0),
        });
        break;
      case "Enter": {
        const item = items[selectedIndex];
        if (!item) break;
        if (e.altKey) {
          // Alt+Enter → Format Paste モーダルを開く
          handleFormatPaste(item.id, item.preview);
        } else {
          handlePaste(item.id, e.shiftKey ? "plain" : "normal");
        }
        break;
      }
      case "Delete":
      case "Backspace": {
        if (state.query === "") {
          const item = items[selectedIndex];
          if (item) handleDelete(item.id);
        }
        break;
      }
      case "Escape":
        if (state.query) {
          handleQueryChange("");
        }
        break;
    }
  }

  return (
    <div className="tab-panel" onKeyDown={handleKeyDown}>
      <SearchBar
        value={state.query}
        onChange={handleQueryChange}
        onClear={() => handleQueryChange("")}
      />

      {state.error && (
        <div className="error-banner" role="alert">
          {state.error}
          <button
            onClick={() => dispatch({ type: "SET_ERROR", error: null })}
          >
            ✕
          </button>
        </div>
      )}

      {state.loading && state.items.length === 0 ? (
        <div className="loading-state">Loading…</div>
      ) : (
        <HistoryList
          items={state.items}
          selectedIndex={state.selectedIndex}
          query={state.query}
          onSelect={(i) => dispatch({ type: "SET_SELECTED", index: i })}
          onPaste={handlePaste}
          onFormatPaste={handleFormatPaste}
          onDelete={handleDelete}
        />
      )}

      <footer className="status-bar">
        <span>{state.items.length} items</span>
        <span className="hint">
          ↑↓ navigate · Enter paste · Shift+Enter plain · Alt+Enter format · ⌘⌫ delete
        </span>
      </footer>

      {formatTarget && (
        <FormatPasteModal
          itemId={formatTarget.id}
          preview={formatTarget.preview}
          onClose={() => setFormatTarget(null)}
          onPasted={() => setFormatTarget(null)}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// App (tab switcher)
// ---------------------------------------------------------------------------

type Tab = "history" | "donelog";

export function App() {
  const [activeTab, setActiveTab] = useState<Tab>("history");

  return (
    <div className="app">
      <nav className="tab-bar" role="tablist">
        <button
          role="tab"
          aria-selected={activeTab === "history"}
          className={`tab-bar__tab ${activeTab === "history" ? "tab-bar__tab--active" : ""}`}
          onClick={() => setActiveTab("history")}
        >
          📋 クリップボード
        </button>
        <button
          role="tab"
          aria-selected={activeTab === "donelog"}
          className={`tab-bar__tab ${activeTab === "donelog" ? "tab-bar__tab--active" : ""}`}
          onClick={() => setActiveTab("donelog")}
        >
          📓 DONE LOG
        </button>
      </nav>

      {activeTab === "history" ? <HistoryTab /> : <DoneLog />}
    </div>
  );
}
