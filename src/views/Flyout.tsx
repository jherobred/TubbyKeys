import { useEffect, useState, type CSSProperties } from "react";
import { api, on } from "../api";
import { useAppState } from "../store";
import { Mascot, SwitchIcon } from "../components/Art";
import { Slider, Toggle } from "../components/Controls";

const order = (i: number) => ({ "--i": i }) as CSSProperties;

/** Tray pop-up: the quick controls from Keeby's menu bar menu. */
export function Flyout() {
  const { snap, error, patch, patchVisualizer } = useAppState();
  // Each open remounts the panel so its entrance animation plays again.
  const [opened, setOpened] = useState(0);
  const [closing, setClosing] = useState(false);

  useEffect(() => {
    const offs = [
      on("flyout-open", () => {
        setClosing(false);
        setOpened((n) => n + 1);
      }),
      on("flyout-close", () => setClosing(true)),
    ];
    return () => offs.forEach((off) => off());
  }, []);

  if (!snap) return <div className="loading">{error ?? "Loading…"}</div>;
  const { settings: s, packs } = snap;

  return (
    <div className="flyout-root">
      <div key={opened} className={`flyout${closing ? " closing" : ""}`}>
        <header className={`flyout-header${s.enabled ? "" : " muted-state"}`} style={order(0)}>
          <Mascot size={30} />
          <div className="flyout-title">
            TubbyKeys
            <span className="muted small">{s.enabled ? "Sounds on" : "Muted"}</span>
          </div>
          <Toggle checked={s.enabled} onChange={(enabled) => patch({ enabled })} label="Sounds on" />
        </header>

        <div className="flyout-section" style={order(1)}>
          <label className="flyout-label">Volume</label>
          <Slider label="Volume" value={s.volume} onChange={(volume) => patch({ volume })} />
          <label className="flyout-label">
            Tone <span className="muted small">thock ↔ clack</span>
          </label>
          <Slider label="Tone" min={-1} max={1} value={s.tone} onChange={(tone) => patch({ tone })} />
        </div>

        <div className="flyout-list" role="radiogroup" aria-label="Switch" style={order(2)}>
          {packs.map((pack) => (
            <button
              key={pack.id}
              type="button"
              role="radio"
              aria-checked={pack.id === s.packId}
              className={`flyout-pack${pack.id === s.packId ? " active" : ""}`}
              onClick={() => patch({ packId: pack.id })}
            >
              <SwitchIcon id={pack.id} size={22} />
              <span>{pack.name}</span>
            </button>
          ))}
        </div>

        <div className="flyout-row" style={order(3)}>
          <span>Visualizer</span>
          <Toggle
            checked={s.visualizer.enabled}
            onChange={(enabled) => patchVisualizer({ enabled })}
            label="Visualizer"
          />
        </div>

        <footer className="flyout-footer" style={order(4)}>
          <button type="button" className="button" onClick={() => api.showSettings()}>
            Settings…
          </button>
          <button type="button" className="link-button" onClick={() => api.quit()}>
            Quit
          </button>
        </footer>
      </div>
    </div>
  );
}
