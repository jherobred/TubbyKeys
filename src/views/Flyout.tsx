import { api } from "../api";
import { useAppState } from "../store";
import { Mascot, SwitchIcon } from "../components/Art";
import { Slider, Toggle } from "../components/Controls";

/** Tray pop-up: the quick controls from Keeby's menu bar menu. */
export function Flyout() {
  const { snap, error, patch, patchVisualizer } = useAppState();
  if (!snap) return <div className="loading">{error ?? "Loading…"}</div>;
  const { settings: s, packs } = snap;

  return (
    <div className="flyout">
      <header className="flyout-header">
        <Mascot size={30} />
        <div className="flyout-title">
          TubbyKeys
          <span className="muted small">{s.enabled ? "Sounds on" : "Muted"}</span>
        </div>
        <Toggle checked={s.enabled} onChange={(enabled) => patch({ enabled })} label="Sounds on" />
      </header>

      <div className="flyout-section">
        <label className="flyout-label">Volume</label>
        <Slider label="Volume" value={s.volume} onChange={(volume) => patch({ volume })} />
        <label className="flyout-label">
          Tone <span className="muted small">thock ↔ clack</span>
        </label>
        <Slider label="Tone" min={-1} max={1} value={s.tone} onChange={(tone) => patch({ tone })} />
      </div>

      <div className="flyout-list" role="radiogroup" aria-label="Switch">
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

      <div className="flyout-row">
        <span>Visualizer</span>
        <Toggle
          checked={s.visualizer.enabled}
          onChange={(enabled) => patchVisualizer({ enabled })}
          label="Visualizer"
        />
      </div>

      <footer className="flyout-footer">
        <button type="button" className="button" onClick={() => api.showSettings()}>
          Settings…
        </button>
        <button type="button" className="link-button" onClick={() => api.quit()}>
          Quit
        </button>
      </footer>
    </div>
  );
}
