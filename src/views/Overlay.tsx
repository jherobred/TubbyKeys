import { useEffect, useRef, useState, type CSSProperties } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, on, type Animation, type KeyPulse, type Settings, type Size, type Visualizer } from "../api";
import { BOARD_WIDTH, KEYS, nearestKey } from "../keyboardLayout";

const UNIT = 15; // px per key width at medium size
const IDLE_AFTER_MS = 2500;
const SCALE: Record<Size, number> = { small: 0.8, medium: 1, large: 1.25 };

const COMBO_ANIMATIONS: Record<Animation, { frames: Keyframe[]; duration: number; easing: string }> = {
  pop: {
    frames: [{ transform: "scale(1.45)" }, { transform: "scale(1)" }],
    duration: 220,
    easing: "cubic-bezier(.2,1.6,.4,1)",
  },
  slide: {
    frames: [
      { transform: "translateY(12px)", opacity: 0.3 },
      { transform: "translateY(0)", opacity: 1 },
    ],
    duration: 180,
    easing: "ease-out",
  },
  bounce: {
    frames: [
      { transform: "translateY(0)" },
      { transform: "translateY(-12px)" },
      { transform: "translateY(0)" },
      { transform: "translateY(-4px)" },
      { transform: "translateY(0)" },
    ],
    duration: 360,
    easing: "ease-out",
  },
  pulse: {
    frames: [
      { transform: "scale(1)", filter: "drop-shadow(0 0 0 rgba(58,209,171,0))" },
      { transform: "scale(1.1)", filter: "drop-shadow(0 0 12px rgba(58,209,171,0.9))" },
      { transform: "scale(1)", filter: "drop-shadow(0 0 0 rgba(58,209,171,0))" },
    ],
    duration: 320,
    easing: "ease-out",
  },
};

/**
 * A line of damped springs coupled to their neighbours: a key press kicks
 * the points under it, the bump spreads out as ripples and settles. Runs
 * only while something is moving.
 */
class WaveSim {
  private readonly n = 96;
  private h = new Float32Array(this.n);
  private v = new Float32Array(this.n);
  private frame = 0;
  private width = 0;
  private baseline = 0;
  private direction = -1;

  constructor(private readonly path: SVGPathElement) {}

  resize(width: number, height: number, edge: "top" | "bottom") {
    this.width = width;
    this.baseline = edge === "bottom" ? height - 12 : 12;
    this.direction = edge === "bottom" ? -1 : 1;
    this.draw();
  }

  kick(x: number, strength: number) {
    const centre = x * (this.n - 1);
    for (let i = 0; i < this.n; i++) {
      const d = (i - centre) / 2.2;
      this.v[i] += strength * 320 * Math.exp(-d * d);
    }
    if (!this.frame) this.frame = requestAnimationFrame(this.tick);
  }

  stop() {
    cancelAnimationFrame(this.frame);
    this.frame = 0;
  }

  private tick = () => {
    const { h, v, n } = this;
    const dt = 1 / 180;
    for (let step = 0; step < 3; step++) {
      for (let i = 0; i < n; i++) {
        const left = i > 0 ? h[i - 1] : h[i];
        const right = i < n - 1 ? h[i + 1] : h[i];
        v[i] += (-120 * h[i] + 900 * (left + right - 2 * h[i]) - 4.5 * v[i]) * dt;
      }
      for (let i = 0; i < n; i++) h[i] += v[i] * dt;
    }
    let energy = 0;
    for (let i = 0; i < n; i++) energy = Math.max(energy, Math.abs(h[i]), Math.abs(v[i]) * 0.02);
    if (energy < 0.15) {
      h.fill(0);
      v.fill(0);
      this.frame = 0;
    } else {
      this.frame = requestAnimationFrame(this.tick);
    }
    this.draw();
  };

  private draw() {
    const { h, n, width, baseline, direction } = this;
    if (!width) return;
    const step = width / (n - 1);
    const y = (i: number) => baseline + direction * Math.max(-44, Math.min(44, h[i]));
    let d = `M 0 ${y(0).toFixed(1)}`;
    for (let i = 1; i < n; i++) {
      const x0 = (i - 1) * step;
      const mx = x0 + step / 2;
      const my = (y(i - 1) + y(i)) / 2;
      d += ` Q ${x0.toFixed(1)} ${y(i - 1).toFixed(1)} ${mx.toFixed(1)} ${my.toFixed(1)}`;
    }
    d += ` L ${width} ${y(n - 1).toFixed(1)}`;
    this.path.setAttribute("d", d);
  }
}

/** Click-through overlay in one of three styles, plus drag-to-move. */
export function Overlay() {
  const [visualizer, setVisualizer] = useState<Visualizer | null>(null);
  const [combo, setCombo] = useState(0);
  const [idle, setIdle] = useState(true);
  const [arranging, setArranging] = useState(false);
  const settingsRef = useRef<Visualizer | null>(null);
  const lastKey = useRef(0);
  const keyRefs = useRef<(HTMLDivElement | null)[]>([]);
  const comboRef = useRef<HTMLDivElement>(null);
  const blobRef = useRef<HTMLDivElement>(null);
  const waveBox = useRef<HTMLDivElement>(null);
  const wavePath = useRef<SVGPathElement>(null);
  const waveGradient = useRef<SVGLinearGradientElement>(null);
  const wave = useRef<WaveSim | null>(null);

  useEffect(() => {
    const apply = (s: Settings) => {
      settingsRef.current = s.visualizer;
      setVisualizer(s.visualizer);
    };
    api.getState().then((snap) => apply(snap.settings));
    const offs = [
      on<Settings>("settings-changed", apply),
      on<boolean>("overlay-arrange", setArranging),
      on<KeyPulse>("key-pulse", (pulse) => {
        const v = settingsRef.current;
        if (!v) return;
        const now = performance.now();
        const expired = v.comboTimeoutMs > 0 && now - lastKey.current > v.comboTimeoutMs;
        lastKey.current = now;
        setIdle(false);
        setCombo((c) => (expired ? 1 : c + 1));

        if (v.style === "keyboard") {
          keyRefs.current[nearestKey(pulse.row, pulse.x)]?.animate(
            [
              {
                backgroundColor: "rgba(58,209,171,0.95)",
                transform: "translateY(1.5px) scale(0.9)",
                boxShadow: "0 0 10px rgba(58,209,171,0.9)",
              },
              { backgroundColor: "rgba(255,255,255,0.16)", transform: "none", boxShadow: "0 0 0 rgba(58,209,171,0)" },
            ],
            { duration: pulse.wide ? 520 : 380, easing: "ease-out" },
          );
        } else if (v.style === "pill") {
          blobRef.current?.animate(
            [{ transform: "scale(1.4, 0.68)" }, { transform: "scale(0.88, 1.14)" }, { transform: "scale(1)" }],
            { duration: 460, easing: "cubic-bezier(.34,1.56,.64,1)" },
          );
        } else {
          wave.current?.kick(pulse.x, pulse.wide ? 1.7 : 1);
        }
        const style = COMBO_ANIMATIONS[v.animation];
        comboRef.current?.animate(style.frames, { duration: style.duration, easing: style.easing });
      }),
    ];
    const timer = window.setInterval(() => {
      const v = settingsRef.current;
      const quiet = performance.now() - lastKey.current;
      if (v && v.comboTimeoutMs > 0 && quiet > v.comboTimeoutMs) setCombo(0);
      if (quiet > IDLE_AFTER_MS) setIdle(true);
    }, 250);
    return () => {
      offs.forEach((off) => off());
      window.clearInterval(timer);
    };
  }, []);

  // The wave simulation lives as long as the wave style is showing.
  const waveEdge = visualizer?.placement === "top" ? "top" : "bottom";
  const isWave = visualizer?.style === "wave";
  useEffect(() => {
    if (!isWave || !wavePath.current || !waveBox.current) return;
    const sim = new WaveSim(wavePath.current);
    wave.current = sim;
    const box = waveBox.current;
    const fit = () => {
      sim.resize(box.clientWidth, box.clientHeight, waveEdge);
      waveGradient.current?.setAttribute("x2", String(box.clientWidth));
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(box);
    return () => {
      observer.disconnect();
      sim.stop();
      wave.current = null;
    };
  }, [isWave, waveEdge]);

  if (!visualizer) return null;
  const v = visualizer;
  const active = !idle || arranging;
  const opacity = active ? 1 : { hide: 0, fade: 0.35, keep: 1 }[v.idle];
  const comboText =
    combo > 0 ? (
      <>
        <span className="combo-x">×</span>
        {combo}
      </>
    ) : (
      " "
    );

  if (v.style === "wave") {
    return (
      <div className="wave-root" ref={waveBox} style={{ opacity }}>
        <svg aria-hidden="true">
          <defs>
            <linearGradient ref={waveGradient} id="wave-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="1000" y2="0">
              <stop offset="0" stopColor="#3ad1ab" stopOpacity="0.05" />
              <stop offset="0.25" stopColor="#3ad1ab" />
              <stop offset="0.75" stopColor="#5aa9ff" />
              <stop offset="1" stopColor="#5aa9ff" stopOpacity="0.05" />
            </linearGradient>
          </defs>
          <path ref={wavePath} className="wave-line" />
        </svg>
        {v.showCombo && combo > 0 && (
          <div className="wave-combo" ref={comboRef} style={waveEdge === "bottom" ? { bottom: 20 } : { top: 20 }}>
            {comboText}
          </div>
        )}
      </div>
    );
  }

  const scale = SCALE[v.size];
  return (
    <div
      className={`overlay${arranging ? " arranging" : ""}`}
      style={{ opacity, "--scale": scale, justifyContent: v.style === "pill" ? "center" : "flex-end" } as CSSProperties}
      onMouseDown={(e) => {
        if (arranging && e.button === 0 && !(e.target instanceof HTMLButtonElement)) {
          getCurrentWindow().startDragging().catch(() => undefined);
        }
      }}
    >
      {arranging && (
        <div className="arrange-bar">
          Drag anywhere
          <button type="button" onClick={() => api.arrangeOverlay(false)}>
            Done
          </button>
        </div>
      )}
      {v.style === "pill" ? (
        <div className="pill">
          <div className="blob" ref={blobRef} />
          <div className="combo" ref={comboRef}>
            {comboText}
          </div>
        </div>
      ) : (
        <>
          {v.showCombo && (
            <div className="combo" ref={comboRef}>
              {comboText}
            </div>
          )}
          <div className="mini-board" style={{ width: BOARD_WIDTH * UNIT, height: 5 * UNIT }}>
            {KEYS.map((key, i) => (
              <div
                key={i}
                ref={(el) => {
                  keyRefs.current[i] = el;
                }}
                className="mini-key"
                style={{
                  left: (key.x - key.w / 2) * UNIT + 1,
                  top: key.row * UNIT + 1,
                  width: key.w * UNIT - 2,
                  height: UNIT - 2,
                }}
              />
            ))}
          </div>
        </>
      )}
    </div>
  );
}
