/**
 * DONE LOG タブ — 作業日誌の閲覧・編集・エクスポート。
 */
import { useCallback, useEffect, useReducer, useRef } from "react";
import {
  captureDone,
  exportDoneMarkdown,
  listDone,
  type DoneViewSummary,
} from "./api";
import { DoneLogItem } from "./DoneLogItem";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

interface State {
  items: DoneViewSummary[];
  date: string; // "YYYY-MM-DD" or "" (today)
  loading: boolean;
  error: string | null;
  capturing: boolean;
  captureInput: string;
  exportText: string | null;
}

type Action =
  | { type: "SET_ITEMS"; items: DoneViewSummary[] }
  | { type: "SET_DATE"; date: string }
  | { type: "SET_LOADING"; loading: boolean }
  | { type: "SET_ERROR"; error: string | null }
  | { type: "SET_CAPTURING"; capturing: boolean }
  | { type: "SET_CAPTURE_INPUT"; value: string }
  | { type: "ITEM_UPDATED"; item: DoneViewSummary }
  | { type: "SET_EXPORT"; text: string | null };

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "SET_ITEMS":
      return { ...state, items: action.items, error: null };
    case "SET_DATE":
      return { ...state, date: action.date };
    case "SET_LOADING":
      return { ...state, loading: action.loading };
    case "SET_ERROR":
      return { ...state, error: action.error };
    case "SET_CAPTURING":
      return { ...state, capturing: action.capturing };
    case "SET_CAPTURE_INPUT":
      return { ...state, captureInput: action.value };
    case "ITEM_UPDATED": {
      const items = state.items.map((i) =>
        i.id === action.item.id ? action.item : i,
      );
      return { ...state, items };
    }
    case "SET_EXPORT":
      return { ...state, exportText: action.text };
  }
}

function todayString() {
  return new Date().toISOString().slice(0, 10);
}

const INITIAL_STATE: State = {
  items: [],
  date: todayString(),
  loading: true,
  error: null,
  capturing: false,
  captureInput: "",
  exportText: null,
};

// ---------------------------------------------------------------------------
// DoneLog component
// ---------------------------------------------------------------------------

export function DoneLog() {
  const [state, dispatch] = useReducer(reducer, INITIAL_STATE);
  const captureRef = useRef<HTMLTextAreaElement>(null);

  const load = useCallback((date: string) => {
    dispatch({ type: "SET_LOADING", loading: true });
    listDone({ date: date || undefined })
      .then((items) => dispatch({ type: "SET_ITEMS", items }))
      .catch((e: unknown) =>
        dispatch({ type: "SET_ERROR", error: String(e) }),
      )
      .finally(() => dispatch({ type: "SET_LOADING", loading: false }));
  }, []);

  // Initial load.
  useEffect(() => {
    load(state.date);
  }, [load, state.date]);

  function handleDateChange(e: React.ChangeEvent<HTMLInputElement>) {
    dispatch({ type: "SET_DATE", date: e.target.value });
  }

  function goToday() {
    dispatch({ type: "SET_DATE", date: todayString() });
  }

  async function handleCapture() {
    const body = state.captureInput.trim();
    if (!body) return;
    dispatch({ type: "SET_CAPTURING", capturing: true });
    dispatch({ type: "SET_ERROR", error: null });
    try {
      await captureDone(body);
      dispatch({ type: "SET_CAPTURE_INPUT", value: "" });
      load(state.date);
    } catch (e) {
      dispatch({ type: "SET_ERROR", error: String(e) });
    } finally {
      dispatch({ type: "SET_CAPTURING", capturing: false });
    }
  }

  async function handleExport() {
    try {
      const md = await exportDoneMarkdown(state.date || undefined);
      dispatch({ type: "SET_EXPORT", text: md });
    } catch (e) {
      dispatch({ type: "SET_ERROR", error: String(e) });
    }
  }

  function handleExportClose() {
    dispatch({ type: "SET_EXPORT", text: null });
  }

  async function handleExportCopy() {
    if (state.exportText) {
      await navigator.clipboard.writeText(state.exportText);
    }
  }

  return (
    <div className="done-log">
      {/* Date navigator */}
      <div className="done-log__toolbar">
        <input
          type="date"
          className="done-log__date-picker"
          value={state.date}
          onChange={handleDateChange}
          max={todayString()}
        />
        <button className="btn btn--sm" onClick={goToday}>
          今日
        </button>
        <span className="done-log__count">{state.items.length} 件</span>
        <button className="btn btn--sm btn--ghost done-log__export-btn" onClick={handleExport}>
          MD エクスポート
        </button>
      </div>

      {/* Error banner */}
      {state.error && (
        <div className="error-banner" role="alert">
          {state.error}
          <button onClick={() => dispatch({ type: "SET_ERROR", error: null })}>
            ✕
          </button>
        </div>
      )}

      {/* Quick-capture box */}
      <div className="done-log__capture">
        <textarea
          ref={captureRef}
          className="done-log__capture-input"
          value={state.captureInput}
          onChange={(e) =>
            dispatch({ type: "SET_CAPTURE_INPUT", value: e.target.value })
          }
          placeholder="作業内容を入力して Ctrl+Enter で記録…"
          rows={2}
          onKeyDown={(e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
              e.preventDefault();
              handleCapture();
            }
          }}
          disabled={state.capturing}
        />
        <button
          className="btn btn--primary done-log__capture-btn"
          onClick={handleCapture}
          disabled={state.capturing || !state.captureInput.trim()}
        >
          {state.capturing ? "記録中…" : "記録"}
        </button>
      </div>

      {/* Entry list */}
      {state.loading && state.items.length === 0 ? (
        <div className="loading-state">Loading…</div>
      ) : state.items.length === 0 ? (
        <div className="done-log__empty">この日の記録はありません</div>
      ) : (
        <ul className="done-log__list">
          {state.items.map((item) => (
            <li key={item.id} className="done-log__list-item">
              <DoneLogItem
                item={item}
                onUpdated={(updated) =>
                  dispatch({ type: "ITEM_UPDATED", item: updated })
                }
              />
            </li>
          ))}
        </ul>
      )}

      {/* Markdown export modal */}
      {state.exportText !== null && (
        <div className="modal-overlay" role="dialog" aria-modal="true">
          <div className="modal">
            <div className="modal__header">
              <h2>Markdown エクスポート — {state.date}</h2>
              <button
                className="modal__close"
                onClick={handleExportClose}
                aria-label="閉じる"
              >
                ✕
              </button>
            </div>
            <pre className="modal__body done-log__export-preview">
              {state.exportText}
            </pre>
            <div className="modal__footer">
              <button className="btn btn--primary" onClick={handleExportCopy}>
                クリップボードにコピー
              </button>
              <button className="btn" onClick={handleExportClose}>
                閉じる
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
