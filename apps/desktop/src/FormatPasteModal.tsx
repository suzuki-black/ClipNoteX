/**
 * Format Paste モーダル
 *
 * クリップボード履歴アイテムを選択した言語でフォーマットし、
 * プレビュー確認後にそのままペーストする。
 *
 * キーボードショートカット:
 *   Ctrl+Enter / Cmd+Enter — Format & Paste
 *   Escape                 — キャンセル
 */
import { useEffect, useRef, useState } from "react";
import { formatPreview, pasteItem, type FormatPreviewResult } from "./api";

// ---------------------------------------------------------------------------
// 型・定数
// ---------------------------------------------------------------------------

export const LANGUAGES = [
  { value: "auto", label: "自動検出" },
  { value: "json", label: "JSON" },
  { value: "sql", label: "SQL" },
  { value: "markdown", label: "Markdown" },
  { value: "plaintext", label: "プレーンテキスト" },
] as const;

type LangValue = (typeof LANGUAGES)[number]["value"];

const INDENT_OPTIONS = [2, 4, 8] as const;

interface Props {
  itemId: string;
  preview: string;
  onClose: () => void;
  onPasted: () => void;
}

// ---------------------------------------------------------------------------
// コンポーネント
// ---------------------------------------------------------------------------

export function FormatPasteModal({ itemId, preview, onClose, onPasted }: Props) {
  const [lang, setLang] = useState<LangValue>("auto");
  const [indent, setIndent] = useState(2);
  const [result, setResult] = useState<FormatPreviewResult | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [pasting, setPasting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // プレビューを生成（lang / indent が変わるたびに debounce で呼ぶ）
  function schedulePreview(nextLang: LangValue, nextIndent: number) {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      setPreviewLoading(true);
      setError(null);
      try {
        const res = await formatPreview(
          preview,
          nextLang === "auto" ? undefined : nextLang,
          nextIndent,
        );
        setResult(res);
      } catch (e) {
        setError(String(e));
        setResult(null);
      } finally {
        setPreviewLoading(false);
      }
    }, 200);
  }

  // 初回マウント時にプレビュー生成
  useEffect(() => {
    schedulePreview(lang, indent);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function handleLangChange(v: LangValue) {
    setLang(v);
    schedulePreview(v, indent);
  }

  function handleIndentChange(v: number) {
    setIndent(v);
    schedulePreview(lang, v);
  }

  async function handlePaste() {
    if (!result) return;
    setPasting(true);
    setError(null);
    try {
      await pasteItem({
        id: itemId,
        mode: "format",
        format_lang: lang === "auto" ? undefined : lang,
        format_indent: indent,
      });
      onPasted();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setPasting(false);
    }
  }

  // キーボードハンドラ
  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      onClose();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      handlePaste();
    }
  }

  const detectedLabel =
    result && lang === "auto"
      ? LANGUAGES.find((l) => l.value === result.detected_lang)?.label ??
        result.detected_lang
      : null;

  return (
    <div
      className="modal-overlay"
      role="dialog"
      aria-modal="true"
      aria-label="書式ペースト"
      onKeyDown={handleKeyDown}
    >
      <div className="modal format-modal">
        {/* ヘッダー */}
        <div className="modal__header">
          <h2>書式ペースト</h2>
          <button
            className="modal__close"
            onClick={onClose}
            aria-label="閉じる"
          >
            ✕
          </button>
        </div>

        {/* オプション行 */}
        <div className="format-modal__options">
          <div className="format-modal__option-group">
            <label className="format-modal__label" htmlFor="fm-lang">
              言語
            </label>
            <select
              id="fm-lang"
              className="format-modal__select"
              value={lang}
              onChange={(e) => handleLangChange(e.target.value as LangValue)}
            >
              {LANGUAGES.map((l) => (
                <option key={l.value} value={l.value}>
                  {l.label}
                </option>
              ))}
            </select>
            {detectedLabel && (
              <span className="format-modal__detected">→ {detectedLabel}</span>
            )}
          </div>

          <div className="format-modal__option-group">
            <label className="format-modal__label" htmlFor="fm-indent">
              インデント
            </label>
            <div className="format-modal__indent-btns" role="group">
              {INDENT_OPTIONS.map((w) => (
                <button
                  key={w}
                  className={`btn btn--sm format-modal__indent-btn ${
                    indent === w ? "btn--primary" : "btn--ghost"
                  }`}
                  onClick={() => handleIndentChange(w)}
                >
                  {w}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* プレビューエリア */}
        <div className="modal__body format-modal__preview-wrap">
          {previewLoading ? (
            <div className="format-modal__preview-loading">フォーマット中…</div>
          ) : error ? (
            <div className="format-modal__error">{error}</div>
          ) : result ? (
            <pre className="format-modal__preview">{result.formatted}</pre>
          ) : (
            <div className="format-modal__preview-loading">—</div>
          )}
        </div>

        {/* フッター */}
        <div className="modal__footer">
          <span className="format-modal__hint">⌘↵ で貼り付け</span>
          <button className="btn" onClick={onClose} disabled={pasting}>
            キャンセル
          </button>
          <button
            className="btn btn--primary"
            onClick={handlePaste}
            disabled={pasting || !result || previewLoading}
          >
            {pasting ? "貼り付け中…" : "書式ペースト"}
          </button>
        </div>
      </div>
    </div>
  );
}
