import { useEffect, useState } from "react";

/** Turn a DOM key code into a token the Rust shortcut parser accepts. */
function keyToken(code: string): string | null {
  if (/^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/.test(code)) return null;
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

export function HotkeyInput(props: { value: string; onChange: (value: string) => void }) {
  const [recording, setRecording] = useState(false);
  const { onChange } = props;

  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape") {
        setRecording(false);
        return;
      }
      const key = keyToken(e.code);
      const modifiers = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(
        (m): m is string => Boolean(m),
      );
      // Wait for a real key, and require a modifier so plain typing never toggles sounds.
      if (!key || modifiers.length === 0) return;
      onChange([...modifiers, key].join("+"));
      setRecording(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, onChange]);

  return (
    <div className="hotkey">
      <button
        type="button"
        className={`hotkey-button${recording ? " recording" : ""}`}
        onClick={() => setRecording((r) => !r)}
        onBlur={() => setRecording(false)}
      >
        {recording ? (
          "Press a shortcut… (Esc to cancel)"
        ) : props.value ? (
          props.value.split("+").map((part) => <kbd key={part}>{part}</kbd>)
        ) : (
          "Not set"
        )}
      </button>
      {props.value && !recording && (
        <button type="button" className="link-button" onClick={() => onChange("")}>
          Clear
        </button>
      )}
    </div>
  );
}
