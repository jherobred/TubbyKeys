import { useState } from "react";
import { api, type Animation, type Idle, type Placement } from "../api";
import { useAppState } from "../store";
import { Mascot } from "../components/Art";
import { Field, Segmented, Slider, Toggle } from "../components/Controls";
import { HotkeyInput } from "../components/HotkeyInput";
import { Marketplace } from "../components/Marketplace";
import { SoundPad } from "../components/SoundPad";
import { SwitchGrid } from "../components/SwitchGrid";

type Page = "sounds" | "visualizer" | "marketplace" | "general";

const PAGES: { id: Page; label: string }[] = [
  { id: "sounds", label: "Sounds" },
  { id: "visualizer", label: "Visualizer" },
  { id: "marketplace", label: "Marketplace" },
  { id: "general", label: "General" },
];

const pct = (v: number) => `${Math.round(v * 100)}%`;

export function SettingsWindow() {
  const { snap, error, setError, patch, patchVisualizer, setPacks } = useAppState();
  const [page, setPage] = useState<Page>("sounds");

  if (!snap) {
    return <div className="loading">{error ?? "Loading…"}</div>;
  }
  const { settings: s, packs, output } = snap;
  const v = s.visualizer;
  const activePack = packs.find((p) => p.id === s.packId);

  return (
    <div className="settings">
      <nav className="sidebar">
        <div className="brand">
          <Mascot size={34} />
          <div>
            <div className="brand-name">TubbyKeys</div>
            <div className="muted small">v{snap.version}</div>
          </div>
        </div>
        {PAGES.map((p) => (
          <button
            key={p.id}
            type="button"
            className={`nav-item${page === p.id ? " active" : ""}`}
            aria-current={page === p.id ? "page" : undefined}
            onClick={() => setPage(p.id)}
          >
            {p.label}
          </button>
        ))}
        <div className="sidebar-footer">
          <span>{s.enabled ? "Sounds on" : "Sounds off"}</span>
          <Toggle checked={s.enabled} onChange={(enabled) => patch({ enabled })} label="Sounds on" />
        </div>
      </nav>

      <main className="content">
        {error && (
          <div className="notice error" role="alert">
            {error}
            <button type="button" className="link-button" onClick={() => setError(null)}>
              Dismiss
            </button>
          </div>
        )}

        {page === "sounds" && (
          <>
            <header className="page-header">
              <h1>Switches</h1>
              <p className="muted">
                Hover to preview, click to use. Now playing: <strong>{activePack?.name ?? s.packId}</strong>
              </p>
            </header>
            <SwitchGrid packs={packs} active={s.packId} onSelect={(packId) => patch({ packId })} />

            <h2>Sound</h2>
            <div className="panel sound-panel">
              <SoundPad tone={s.tone} pitch={s.pitch} onChange={(tone, pitch) => patch({ tone, pitch })} />
              <div className="sound-fields">
                <Field title="Volume" hint={pct(s.volume)}>
                  <Slider label="Volume" value={s.volume} onChange={(volume) => patch({ volume })} />
                </Field>
                <Field title="Randomize pitch" hint="Tiny pitch and level changes so repeated keys sound natural.">
                  <Toggle
                    label="Randomize pitch"
                    checked={s.randomizePitch}
                    onChange={(randomizePitch) => patch({ randomizePitch })}
                  />
                </Field>
                <Field title="Spatial audio" hint="Keys on the left sound from the left, keys on the right from the right.">
                  <Toggle label="Spatial audio" checked={s.spatial} onChange={(spatial) => patch({ spatial })} />
                </Field>
                <Field title="Stereo width" hint={pct(s.stereoWidth)}>
                  <Slider
                    label="Stereo width"
                    value={s.stereoWidth}
                    disabled={!s.spatial}
                    onChange={(stereoWidth) => patch({ stereoWidth })}
                  />
                </Field>
                <Field
                  title="Narrow on headphones"
                  hint={
                    output.device
                      ? `Output: ${output.device}${output.headphones ? " (headphones detected)" : ""}`
                      : "No output device found"
                  }
                >
                  <Toggle
                    label="Narrow on headphones"
                    checked={s.headphoneWidth}
                    disabled={!s.spatial}
                    onChange={(headphoneWidth) => patch({ headphoneWidth })}
                  />
                </Field>
              </div>
            </div>
          </>
        )}

        {page === "visualizer" && (
          <>
            <header className="page-header">
              <h1>Visualizer</h1>
              <p className="muted">
                A small overlay that ripples with every key and counts your combo. It never takes focus and clicks pass
                through it.
              </p>
            </header>
            <div className="panel">
              <Field
                title="Show visualizer"
                hint="It shows which keys you press, so screen shares and recordings can see it."
              >
                <Toggle label="Show visualizer" checked={v.enabled} onChange={(enabled) => patchVisualizer({ enabled })} />
              </Field>
              <Field title="Keyboard">
                <Toggle
                  label="Show keyboard"
                  checked={v.showKeyboard}
                  onChange={(showKeyboard) => patchVisualizer({ showKeyboard })}
                />
              </Field>
              <Field title="Combo counter">
                <Toggle label="Show combo" checked={v.showCombo} onChange={(showCombo) => patchVisualizer({ showCombo })} />
              </Field>
              <Field title="Placement" hint="Random hops around the screen as you type.">
                <Segmented<Placement>
                  label="Placement"
                  value={v.placement}
                  onChange={(placement) => patchVisualizer({ placement })}
                  options={[
                    { value: "top", label: "Top" },
                    { value: "bottom", label: "Bottom" },
                    { value: "random", label: "Random" },
                  ]}
                />
              </Field>
              <Field title="Animation">
                <Segmented<Animation>
                  label="Animation"
                  value={v.animation}
                  onChange={(animation) => patchVisualizer({ animation })}
                  options={[
                    { value: "pop", label: "Pop In" },
                    { value: "slide", label: "Slide" },
                    { value: "bounce", label: "Bounce" },
                    { value: "pulse", label: "Pulse" },
                  ]}
                />
              </Field>
              <Field title="Combo resets after" hint="Forever keeps counting until you quit.">
                <Segmented<number>
                  label="Combo timeout"
                  value={v.comboTimeoutMs}
                  onChange={(comboTimeoutMs) => patchVisualizer({ comboTimeoutMs })}
                  options={[
                    { value: 1000, label: "1 s" },
                    { value: 2000, label: "2 s" },
                    { value: 5000, label: "5 s" },
                    { value: 0, label: "Forever" },
                  ]}
                />
              </Field>
              <Field title="When idle">
                <Segmented<Idle>
                  label="When idle"
                  value={v.idle}
                  onChange={(idle) => patchVisualizer({ idle })}
                  options={[
                    { value: "hide", label: "Hide" },
                    { value: "fade", label: "Fade" },
                    { value: "keep", label: "Keep" },
                  ]}
                />
              </Field>
            </div>
          </>
        )}

        {page === "marketplace" && (
          <>
            <header className="page-header">
              <h1>Marketplace</h1>
              <p className="muted">Community sound packs, free to download.</p>
            </header>
            <Marketplace
              packs={packs}
              active={s.packId}
              onPacks={setPacks}
              onSelect={(packId) => patch({ packId })}
            />
          </>
        )}

        {page === "general" && (
          <>
            <header className="page-header">
              <h1>General</h1>
            </header>
            <div className="panel">
              <Field title="Toggle sounds shortcut" hint="Works from any app.">
                <HotkeyInput value={s.hotkey} onChange={(hotkey) => patch({ hotkey })} />
              </Field>
            </div>
            <h2>Privacy</h2>
            <div className="panel prose">
              <ul>
                <li>TubbyKeys reads only which physical key moved and whether it went down or up.</li>
                <li>It never reads, records, stores or sends what you type.</li>
                <li>No accounts, analytics or tracking. Settings and packs stay on this PC.</li>
                <li>The only network use is the Marketplace page, which downloads from GitHub while it is open.</li>
              </ul>
            </div>
            <h2>About</h2>
            <div className="panel prose">
              <p>
                TubbyKeys is free and open source under the MIT license. Built-in switch recordings are from{" "}
                <em>kbsim</em> by Thomas Lai (MIT). Marketplace packs list their own authors and licenses.
              </p>
              <div className="button-row">
                <button type="button" className="button" onClick={() => api.openLink("repo")}>
                  GitHub repository
                </button>
                <button type="button" className="button" onClick={() => api.openLink("issues")}>
                  Report a problem
                </button>
                <button type="button" className="button danger" onClick={() => api.quit()}>
                  Quit TubbyKeys
                </button>
              </div>
            </div>
          </>
        )}
      </main>
    </div>
  );
}
