import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
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

async function hideWindow() {
  try {
    await getCurrentWindow().hide();
  } catch (e) {
    console.error("hide window failed:", e);
  }
}

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
    case "SET_ITEMS": {
      // 選択中のアイテムが新リストにも存在するならその位置に追従、
      // なければ範囲内にクランプ。selectedIndex を 0 にリセットしない
      // (1秒ポーリング中にカーソルが勝手に上に飛ぶのを防ぐ)。
      const currentId = state.items[state.selectedIndex]?.id;
      let nextIndex = currentId
        ? action.items.findIndex((i) => i.id === currentId)
        : state.selectedIndex;
      if (nextIndex < 0) nextIndex = state.selectedIndex;
      nextIndex = Math.min(nextIndex, action.items.length - 1);
      nextIndex = Math.max(0, nextIndex);
      return {
        ...state,
        items: action.items,
        selectedIndex: nextIndex,
        error: null,
      };
    }
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
  // 最新 state を ref に保持: document.keydown ハンドラから常に最新値を参照できる
  const stateRef = useRef(state);
  stateRef.current = state;

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

  // ── グローバルキーバインド (document レベル) ──────────────────────────────
  // 以前は div の onKeyDown だったが、div はフォーカス不可で
  //   1) ウィンドウ表示直後にどこにもフォーカスが当たっていないと Enter が届かない
  //   2) 検索入力中も Enter/矢印を奪う必要がある
  // のため document に集約する。state は ref 経由で常に最新を参照。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat) return;
      const { items, selectedIndex, query } = stateRef.current;
      const target = e.target as HTMLElement | null;
      const inSearchInput =
        target?.tagName === "INPUT" || target?.tagName === "TEXTAREA";

      console.log("[ClipNoteX] keydown", { key: e.key, meta: e.metaKey, shift: e.shiftKey, alt: e.altKey, inSearchInput });

      // Cmd+Shift+V → ウィンドウを隠す
      if (e.metaKey && e.shiftKey && (e.key === "V" || e.key === "v")) {
        e.preventDefault();
        e.stopPropagation();
        hideWindow();
        return;
      }
      // Escape: 検索があればクリア、なければウィンドウを隠す
      if (e.key === "Escape") {
        e.preventDefault();
        if (query) {
          handleQueryChange("");
        } else {
          hideWindow();
        }
        return;
      }
      // ↓ 選択を下へ
      if (e.key === "ArrowDown") {
        e.preventDefault();
        dispatch({
          type: "SET_SELECTED",
          index: Math.min(selectedIndex + 1, items.length - 1),
        });
        return;
      }
      // ↑ 選択を上へ
      if (e.key === "ArrowUp") {
        e.preventDefault();
        dispatch({
          type: "SET_SELECTED",
          index: Math.max(selectedIndex - 1, 0),
        });
        return;
      }
      // Enter → ペースト (Shift=plain / Alt=Format)
      if (e.key === "Enter") {
        const item = items[selectedIndex];
        console.log("[ClipNoteX] Enter pressed", { selectedIndex, hasItem: !!item, itemsLen: items.length });
        if (!item) return;
        e.preventDefault();
        if (e.altKey) {
          handleFormatPaste(item.id, item.preview);
        } else {
          handlePaste(item.id, e.shiftKey ? "plain" : "normal");
        }
        return;
      }
      // Backspace/Delete: 検索が空 (かつ入力中でない) ときだけ削除
      if ((e.key === "Backspace" || e.key === "Delete") && !inSearchInput && !query) {
        const item = items[selectedIndex];
        if (item) {
          e.preventDefault();
          handleDelete(item.id);
        }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // バックグラウンドで新しいクリップボード項目がキャプチャされてもUIに反映されないので、
  // (a) ウィンドウがフォーカスを得たとき、(b) 1秒ごと(可視時のみ) で再ロード。
  // 検索クエリ中は最新検索結果を維持するため query で再ロード。
  useEffect(() => {
    const reload = () => {
      if (document.visibilityState === "visible") {
        load(state.query);
      }
    };
    // フォーカス取得 = ユーザがウィンドウを呼び出した瞬間。
    // 最新コピー(リスト先頭)を初期選択にし、search input にフォーカスを当て直す
    // (Tauri の set_focus だけだと WebView 内のフォーカスは戻らないことがある)。
    const focusSearchInput = () => {
      // 次のフレームを待ってフォーカス。WebKit の都合で即時 focus() が無視されるケース対策。
      requestAnimationFrame(() => {
        const el = document.querySelector<HTMLInputElement>("input.search-input");
        console.log("[ClipNoteX] focusSearchInput, found:", !!el);
        el?.focus();
      });
    };
    const onFocus = () => {
      console.log("[ClipNoteX] window focus event");
      dispatch({ type: "SET_SELECTED", index: 0 });
      focusSearchInput();
      reload();
    };
    const onVisibility = () => {
      console.log("[ClipNoteX] visibilitychange", document.visibilityState);
      if (document.visibilityState === "visible") {
        dispatch({ type: "SET_SELECTED", index: 0 });
        focusSearchInput();
      }
      reload();
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);
    const intervalId = window.setInterval(reload, 1000);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
      window.clearInterval(intervalId);
    };
  }, [load, state.query]);

  function handleQueryChange(query: string) {
    dispatch({ type: "SET_QUERY", query });
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => load(query), 200);
  }

  async function handlePaste(id: string, mode: "normal" | "plain") {
    try {
      // 🔑 重要: ペースト前にウィンドウを隠す。
      // ウィンドウが key window のままだと、合成された Cmd+V は ClipNoteX 自身に飛ぶ。
      // 隠すことでフォーカスが呼び出し元 (テキストエディタ等) に戻り、そこに貼り付くようになる。
      console.log("[ClipNoteX] handlePaste: hiding window first", { id, mode });
      await hideWindow();
      // OS がフォーカス移行を処理する時間を与える (50ms)
      await new Promise((r) => setTimeout(r, 80));
      console.log("[ClipNoteX] handlePaste: calling pasteItem");
      await pasteItem({ id, mode });
      console.log("[ClipNoteX] handlePaste: done");
    } catch (e) {
      console.error("[ClipNoteX] handlePaste error:", e);
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

  // (handleKeyDown は document 級のイベントハンドラに移行済み)

  return (
    <div className="tab-panel">
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
        {/* 一時診断用: クリック=フォーカス無関係に paste を直接叩く */}
        <button
          style={{ marginLeft: 8, fontSize: 11 }}
          onClick={() => {
            const item = state.items[state.selectedIndex];
            console.log("[ClipNoteX] DEBUG paste button clicked", { item });
            if (item) handlePaste(item.id, "normal");
          }}
          title="デバッグ用: 選択中アイテムを paste"
        >
          ▶ Paste
        </button>
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
