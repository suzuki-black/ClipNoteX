import { useRef } from "react";
import { enableInputFocus } from "./api";

interface Props {
  value: string;
  onChange: (v: string) => void;
  onClear: () => void;
}

/**
 * Search bar.
 *
 * DESIGN §7.2: 最初の入力時に enable_input_focus を呼び出し、
 * NSPanel を通常のキーウィンドウに昇格させて IME を有効化する。
 */
export function SearchBar({ value, onChange, onClear }: Props) {
  const focusSentRef = useRef(false);

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    // IME deferred activation: send only on first keystroke.
    if (!focusSentRef.current && e.target.value.length > 0) {
      focusSentRef.current = true;
      enableInputFocus().catch(() => {/* best-effort */});
    }
    onChange(e.target.value);
  }

  return (
    <div className="search-bar">
      <span className="search-icon" aria-hidden>🔍</span>
      <input
        type="search"
        className="search-input"
        placeholder="Search clipboard history…"
        value={value}
        onChange={handleChange}
        autoFocus
        aria-label="Search clipboard history"
      />
      {value && (
        <button
          className="search-clear"
          onClick={onClear}
          aria-label="Clear search"
        >
          ✕
        </button>
      )}
    </div>
  );
}
