import { useRef, type KeyboardEvent, type PointerEvent } from "react";

const clamp = (v: number) => Math.max(-1, Math.min(1, v));
const round = (v: number) => Math.round(v * 100) / 100;

function describe(value: number, low: string, high: string) {
  if (Math.abs(value) < 0.05) return "neutral";
  return `${Math.round(Math.abs(value) * 100)}% ${value < 0 ? low : high}`;
}

/** 2D pad: left/right is tone (thock to clack), down/up is pitch (deep to sharp). */
export function SoundPad(props: { tone: number; pitch: number; onChange: (tone: number, pitch: number) => void }) {
  const ref = useRef<HTMLDivElement>(null);

  const setFromPointer = (e: PointerEvent<HTMLDivElement>) => {
    const rect = ref.current?.getBoundingClientRect();
    if (!rect) return;
    const tone = clamp(((e.clientX - rect.left) / rect.width) * 2 - 1);
    const pitch = clamp(1 - ((e.clientY - rect.top) / rect.height) * 2);
    props.onChange(round(tone), round(pitch));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 0.25 : 0.05;
    const moves: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, step],
      ArrowDown: [0, -step],
    };
    if (e.key === "Home" || e.key === "0") {
      props.onChange(0, 0);
    } else if (moves[e.key]) {
      const [dx, dy] = moves[e.key];
      props.onChange(round(clamp(props.tone + dx)), round(clamp(props.pitch + dy)));
    } else {
      return;
    }
    e.preventDefault();
  };

  return (
    <div className="pad-wrap">
      <div
        ref={ref}
        className="pad"
        role="slider"
        tabIndex={0}
        aria-label="Tone and pitch pad. Arrow keys move, Home resets."
        aria-valuetext={`Tone ${describe(props.tone, "thock", "clack")}, pitch ${describe(props.pitch, "deep", "sharp")}`}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          setFromPointer(e);
        }}
        onPointerMove={(e) => {
          if (e.currentTarget.hasPointerCapture(e.pointerId)) setFromPointer(e);
        }}
        onDoubleClick={() => props.onChange(0, 0)}
        onKeyDown={onKeyDown}
      >
        <span className="pad-axis horizontal" />
        <span className="pad-axis vertical" />
        <span className="pad-label top">Sharp</span>
        <span className="pad-label bottom">Deep</span>
        <span className="pad-label left">Thock</span>
        <span className="pad-label right">Clack</span>
        <span className="pad-puck" style={{ left: `${(props.tone + 1) * 50}%`, top: `${(1 - props.pitch) * 50}%` }} />
      </div>
      <div className="pad-readout">
        <span>Tone: {describe(props.tone, "thock", "clack")}</span>
        <span>Pitch: {describe(props.pitch, "deep", "sharp")}</span>
        <button type="button" className="link-button" onClick={() => props.onChange(0, 0)}>
          Reset
        </button>
      </div>
    </div>
  );
}
