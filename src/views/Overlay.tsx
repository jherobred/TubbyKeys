import { useEffect, useRef, useState } from "react";
import { api, on, type Animation, type KeyPulse, type Settings, type Visualizer } from "../api";
import { BOARD_WIDTH, KEYS, nearestKey } from "../keyboardLayout";

const UNIT = 15; // px per key width
const IDLE_AFTER_MS = 2500;

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

/** Click-through overlay: a mini keyboard that ripples, plus the combo counter. */
export function Overlay() {
  const [visualizer, setVisualizer] = useState<Visualizer | null>(null);
  const [combo, setCombo] = useState(0);
  const [idle, setIdle] = useState(true);
  const keyRefs = useRef<(HTMLDivElement | null)[]>([]);
  const comboRef = useRef<HTMLDivElement>(null);
  const settingsRef = useRef<Visualizer | null>(null);
  const lastKey = useRef(0);

  useEffect(() => {
    const apply = (s: Settings) => {
      settingsRef.current = s.visualizer;
      setVisualizer(s.visualizer);
    };
    api.getState().then((snap) => apply(snap.settings));
    const offSettings = on<Settings>("settings-changed", apply);
    const offPulse = on<KeyPulse>("key-pulse", (pulse) => {
      const v = settingsRef.current;
      if (!v) return;
      const now = performance.now();
      const expired = v.comboTimeoutMs > 0 && now - lastKey.current > v.comboTimeoutMs;
      lastKey.current = now;
      setIdle(false);
      setCombo((c) => (expired ? 1 : c + 1));

      const key = keyRefs.current[nearestKey(pulse.row, pulse.x)];
      key?.animate(
        [
          { backgroundColor: "rgba(58,209,171,0.95)", transform: "translateY(1.5px) scale(0.92)", boxShadow: "0 0 10px rgba(58,209,171,0.9)" },
          { backgroundColor: "rgba(255,255,255,0.16)", transform: "none", boxShadow: "0 0 0 rgba(58,209,171,0)" },
        ],
        { duration: pulse.wide ? 520 : 380, easing: "ease-out" },
      );
      const style = COMBO_ANIMATIONS[v.animation];
      comboRef.current?.animate(style.frames, { duration: style.duration, easing: style.easing });
    });

    const timer = window.setInterval(() => {
      const v = settingsRef.current;
      const quiet = performance.now() - lastKey.current;
      if (v && v.comboTimeoutMs > 0 && quiet > v.comboTimeoutMs) setCombo(0);
      if (quiet > IDLE_AFTER_MS) setIdle(true);
    }, 250);

    return () => {
      offSettings();
      offPulse();
      window.clearInterval(timer);
    };
  }, []);

  if (!visualizer) return null;
  const opacity = idle ? { hide: 0, fade: 0.35, keep: 1 }[visualizer.idle] : 1;

  return (
    <div className="overlay" style={{ opacity }}>
      {visualizer.showCombo && (
        <div className="combo" ref={comboRef} aria-hidden={combo === 0}>
          {combo > 0 ? (
            <>
              <span className="combo-x">×</span>
              {combo}
            </>
          ) : (
            " "
          )}
        </div>
      )}
      {visualizer.showKeyboard && (
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
      )}
    </div>
  );
}
