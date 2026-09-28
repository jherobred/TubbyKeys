import type { CSSProperties, ReactNode } from "react";

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

export function Segmented<T extends string | number>(props: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <div className="segmented" role="radiogroup" aria-label={props.label}>
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
