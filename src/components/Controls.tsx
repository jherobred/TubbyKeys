import { useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";

export function Toggle(props: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={props.checked}
      aria-label={props.label}
      disabled={props.disabled}
      className={`toggle${props.checked ? " on" : ""}`}
      onClick={() => props.onChange(!props.checked)}
    >
      <span className="knob" />
    </button>
  );
}

export function Slider(props: {
  value: number;
  onChange: (value: number) => void;
  label: string;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
}) {
  const { min = 0, max = 1, step = 0.01 } = props;
  const fill = ((props.value - min) / (max - min)) * 100;
  return (
    <input
      type="range"
      className="slider"
      aria-label={props.label}
      min={min}
      max={max}
      step={step}
      value={props.value}
      disabled={props.disabled}
      style={{ "--fill": `${fill}%` } as CSSProperties}
      onChange={(e) => props.onChange(Number(e.target.value))}
    />
  );
}

export function describeBalance(value: number) {
  if (Math.abs(value) < 0.01) return "Middle";
  return `${Math.round(Math.abs(value) * 100)}% ${value < 0 ? "left" : "right"}`;
}

/** Left/right balance. The fill grows out from the middle; double-click centres it. */
export function BalanceSlider(props: { value: number; onChange: (value: number) => void }) {
  const at = (props.value + 1) * 50;
  return (
    <div className="balance">
      <input
        type="range"
        className="slider balance-slider"
        aria-label="Balance"
        aria-valuetext={describeBalance(props.value)}
        min={-1}
        max={1}
        step={0.01}
        value={props.value}
        style={{ "--from": `${Math.min(50, at)}%`, "--to": `${Math.max(50, at)}%` } as CSSProperties}
        onChange={(e) => {
          const value = Number(e.target.value);
          // Snap to the middle so it is easy to hit exactly.
          props.onChange(Math.abs(value) < 0.05 ? 0 : value);
        }}
        onDoubleClick={() => props.onChange(0)}
      />
      <div className="balance-labels" aria-hidden="true">
        <span>Left ←</span>
        <span>Middle</span>
        <span>→ Right</span>
      </div>
    </div>
  );
}

/** Option picker whose highlight glides to the selected option. */
export function Segmented<T extends string | number>(props: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  label: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [thumb, setThumb] = useState<{ x: number; width: number; ready: boolean } | null>(null);
  const optionKey = props.options.map((o) => `${o.value}:${o.label}`).join("|");

  useLayoutEffect(() => {
    const active = ref.current?.querySelector<HTMLElement>('[aria-checked="true"]');
    if (!active) {
      setThumb(null);
      return;
    }
    const x = active.offsetLeft;
    const width = active.offsetWidth;
    // The first placement snaps; later ones animate.
    setThumb((prev) =>
      prev && prev.x === x && prev.width === width ? prev : { x, width, ready: prev !== null },
    );
  }, [props.value, optionKey]);

  return (
    <div ref={ref} className="segmented" role="radiogroup" aria-label={props.label}>
      {thumb && (
        <span
          className={`segmented-thumb${thumb.ready ? " ready" : ""}`}
          style={{ transform: `translateX(${thumb.x}px)`, width: thumb.width }}
        />
      )}
      {props.options.map((option) => (
        <button
          key={String(option.value)}
          type="button"
          role="radio"
          aria-checked={option.value === props.value}
          className={option.value === props.value ? "active" : ""}
          onClick={() => props.onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function Field(props: { title: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="field">
      <div className="field-text">
        <div className="field-title">{props.title}</div>
        {props.hint && <div className="field-hint">{props.hint}</div>}
      </div>
      <div className="field-control">{props.children}</div>
    </div>
  );
}
