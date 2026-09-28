import { useEffect, useState } from "react";
import { api, on, type Animation, type Idle, type Placement, type Size, type Style } from "../api";
import { useAppState } from "../store";
import { Mascot } from "../components/Art";
import { BalanceSlider, describeBalance, Field, Segmented, Slider, Toggle } from "../components/Controls";
import { HotkeyInput } from "../components/HotkeyInput";
import { Marketplace } from "../components/Marketplace";
import { SoundPad } from "../components/SoundPad";
import { SwitchGrid } from "../components/SwitchGrid";
import { TrayTip } from "../components/TrayTip";

type Page = "sounds" | "visualizer" | "marketplace" | "general";

const PAGES: { id: Page; label: string }[] = [
  { id: "sounds", label: "Sounds" },
  { id: "visualizer", label: "Visualizer" },
  { id: "marketplace", label: "Marketplace" },
  { id: "general", label: "General" },
];

/** Nav items are 36px tall with a 4px gap; the highlight glides between them. */
const NAV_STEP = 40;

const STYLE_HINTS: Record<Style, string> = {
  keyboard: "A mini keyboard that ripples where you type, with a combo counter.",
  pill: "Just the combo, in a small capsule. The least in your way.",
  wave: "A thin glowing line along the screen edge that ripples where you type.",
};

const pct = (v: number) => `${Math.round(v * 100)}%`;

export function SettingsWindow() {
  const { snap, error, setError, patch, patchVisualizer, setPacks } = useAppState();
  const [page, setPage] = useState<Page>("sounds");
  const [arranging, setArranging] = useState(false);

  // The overlay's own Done button also ends arranging.
  useEffect(() => on<boolean>("overlay-arrange", setArranging), []);

  if (!snap) {
    return (
      <div className="loading">
        <Mascot size={48} />
        {error ?? "Loading…"}
      </div>
    );
  }
  const { settings: s, packs, output } = snap;
  const v = s.visualizer;
  const activePack = packs.find((p) => p.id === s.packId);
  const pageIndex = PAGES.findIndex((p) => p.id === page);

  const arrange = (active: boolean) => {
    api.arrangeOverlay(active).then(
      () => setArranging(active),
      (e) => setError(String(e)),
    );
  };

  const placements: { value: Placement; label: string }[] =
    v.style === "wave"
      ? [
          { value: "top", label: "Top edge" },
          { value: "bottom", label: "Bottom edge" },
        ]
      : [
          { value: "top", label: "Top" },
          { value: "bottom", label: "Bottom" },
          { value: "random", label: "Random" },
          ...(v.placement === "custom" ? [{ value: "custom" as Placement, label: "Custom" }] : []),
        ];

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
        <div className="nav">
          <span className="nav-indicator" style={{ transform: `translateY(${pageIndex * NAV_STEP}px)` }} />
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
        </div>
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
          <div className="page" key="sounds">
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
                <Field title="Balance" hint={`${describeBalance(s.balance)}. Double-click to centre.`}>
                  <BalanceSlider value={s.balance} onChange={(balance) => patch({ balance })} />
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
          </div>
        )}

        {page === "visualizer" && (
          <div className="page" key="visualizer">
            <header className="page-header">
              <h1>Visualizer</h1>
              <p className="muted">
                Shows your typing without getting in the way: clicks pass through it and it never takes focus.
              </p>
            </header>
            <div className="panel">
              <Field title="Show visualizer">
                <Toggle label="Show visualizer" checked={v.enabled} onChange={(enabled) => patchVisualizer({ enabled })} />
              </Field>
              <Field title="Style" hint={STYLE_HINTS[v.style]}>
                <Segmented<Style>
                  label="Style"
                  value={v.style}
                  onChange={(style) => patchVisualizer({ style })}
                  options={[
                    { value: "keyboard", label: "Keyboard" },
                    { value: "pill", label: "Pill" },
                    { value: "wave", label: "Wave" },
                  ]}
                />
              </Field>
              {v.style !== "wave" && (
                <Field title="Size">
                  <Segmented<Size>
                    label="Size"
                    value={v.size}
                    onChange={(size) => patchVisualizer({ size })}
                    options={[
                      { value: "small", label: "S" },
                      { value: "medium", label: "M" },
                      { value: "large", label: "L" },
                    ]}
                  />
                </Field>
              )}
              <Field
                title="Placement"
                hint={v.style === "wave" ? "The wave runs along a screen edge." : "Or drag it anywhere, on any monitor."}
              >
                <Segmented<Placement>
                  label="Placement"
                  value={v.placement}
                  onChange={(placement) => patchVisualizer({ placement })}
                  options={placements}
                />
                {v.style !== "wave" && (
                  <button
                    type="button"
                    className={`button${arranging ? " primary" : ""}`}
                    disabled={!v.enabled}
                    onClick={() => arrange(!arranging)}
                  >
                    {arranging ? "Done" : "Move…"}
                  </button>
                )}
              </Field>
              <Field title="Combo counter">
                <Toggle label="Show combo" checked={v.showCombo} onChange={(showCombo) => patchVisualizer({ showCombo })} />
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
            <h2>Stay out of the way</h2>
            <div className="panel">
              <Field title="Hide during full-screen apps" hint="Games, videos and presentations.">
                <Toggle
                  label="Hide during full-screen apps"
                  checked={v.hideInFullscreen}
                  onChange={(hideInFullscreen) => patchVisualizer({ hideInFullscreen })}
                />
              </Field>
              <Field
                title="Hide from screen capture"
                hint="Screenshots, recordings and screen shares won't show it. Turn off to show it on stream."
              >
                <Toggle
                  label="Hide from screen capture"
                  checked={v.hideFromCapture}
                  onChange={(hideFromCapture) => patchVisualizer({ hideFromCapture })}
                />
              </Field>
            </div>
          </div>
        )}

        {page === "marketplace" && (
          <div className="page" key="marketplace">
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
          </div>
        )}

        {page === "general" && (
          <div className="page" key="general">
            <header className="page-header">
              <h1>General</h1>
            </header>
            <div className="panel">
              <Field title="Toggle sounds shortcut" hint="Works from any app.">
                <HotkeyInput value={s.hotkey} onChange={(hotkey) => patch({ hotkey })} />
              </Field>
            </div>
            <h2>Tray icon</h2>
            <div className="panel">
              <TrayTip />
              <Field title="React to typing" hint="The icon squishes with every key press.">
                <Toggle label="React to typing" checked={s.trayPulse} onChange={(trayPulse) => patch({ trayPulse })} />
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
          </div>
        )}
      </main>
    </div>
  );
}
