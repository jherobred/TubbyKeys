import { useCallback, useEffect, useState, type CSSProperties } from "react";
import { api, type MarketItem, type PackInfo } from "../api";
import { SwitchIcon } from "./Art";

const Check = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
    <circle cx="8" cy="8" r="8" fill="currentColor" opacity="0.18" />
    <path d="M4.5 8.2 7 10.6l4.6-5" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const kb = (bytes: number) => (bytes >= 1_000_000 ? `${(bytes / 1_000_000).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1000))} KB`);

export function Marketplace(props: {
  packs: PackInfo[];
  active: string;
  onPacks: (packs: PackInfo[]) => void;
  onSelect: (id: string) => void;
}) {
  const [items, setItems] = useState<MarketItem[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const load = useCallback(() => {
    setLoading(true);
    setError(null);
    api
      .marketList()
      .then(setItems, (e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  // Opening this page is the only thing that connects TubbyKeys to the internet.
  useEffect(() => load(), [load]);

  const run = async (id: string, action: () => Promise<PackInfo[]>, installed: string | null) => {
    setBusy(id);
    setError(null);
    try {
      props.onPacks(await action());
      setItems((list) => list?.map((i) => (i.id === id ? { ...i, installedVersion: installed } : i)) ?? null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const q = query.trim().toLowerCase();
  const visible = (items ?? []).filter((i) =>
    [i.name, i.author, i.type, i.description].some((field) => field.toLowerCase().includes(q)),
  );
  const local = props.packs.filter((p) => !p.builtin && !items?.some((i) => i.id === p.id));

  return (
    <div className="market">
      <div className="market-bar">
        <input
          className="search"
          type="search"
          placeholder="Search packs"
          aria-label="Search packs"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button type="button" className="button" onClick={load} disabled={loading}>
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>
      <p className="muted small">
        Packs come from the community catalogue on GitHub. Each file is checked against its SHA-256 checksum before it
        is installed. TubbyKeys only goes online while this page loads.
      </p>
      {error && <div className="notice error">{error}</div>}
      {items && visible.length === 0 && !loading && <p className="muted">No packs match.</p>}
      <div className="market-list">
        {visible.map((item, i) => {
          const installed = item.installedVersion !== null;
          const update = installed && item.installedVersion !== item.version;
          return (
            <article key={item.id} className="market-card" style={{ "--i": i } as CSSProperties}>
              <SwitchIcon id={item.id} size={40} />
              <div className="market-info">
                <div className="market-title">
                  {item.name}
                  <span className="badge">{item.type || "pack"}</span>
                </div>
                <div className="muted small">
                  by {item.author || "unknown"} · {item.license || "no license"} · {kb(item.size)}
                  {item.version && ` · v${item.version}`}
                </div>
                {item.description && <p className="market-desc">{item.description}</p>}
              </div>
              <div className="market-actions">
                {!installed || update ? (
                  <button
                    type="button"
                    className={`button primary${busy === item.id ? " busy" : ""}`}
                    disabled={busy !== null}
                    onClick={() => run(item.id, () => api.marketInstall(item.id), item.version)}
                  >
                    {busy === item.id ? "Installing…" : update ? "Update" : "Install"}
                  </button>
                ) : (
                  <>
                  <span className="installed">
                    <Check /> Installed
                  </span>
                  <button
                    type="button"
                    className="button"
                    disabled={item.id === props.active}
                    onClick={() => props.onSelect(item.id)}
                  >
                    {item.id === props.active ? "In use" : "Use"}
                  </button>
                  </>
                )}
                {installed && (
                  <button
                    type="button"
                    className="link-button danger"
                    disabled={busy !== null}
                    onClick={() => run(item.id, () => api.removePack(item.id), null)}
                  >
                    Remove
                  </button>
                )}
              </div>
            </article>
          );
        })}
      </div>
      {local.length > 0 && (
        <>
          <h3 className="group-title">Installed from your packs folder</h3>
          <div className="market-list">
            {local.map((pack, i) => (
              <article key={pack.id} className="market-card" style={{ "--i": i } as CSSProperties}>
                <SwitchIcon id={pack.id} size={40} />
                <div className="market-info">
                  <div className="market-title">{pack.name}</div>
                  <div className="muted small">
                    by {pack.author || "unknown"} · {pack.license || "no license"}
                  </div>
                </div>
                <div className="market-actions">
                  <button
                    type="button"
                    className="link-button danger"
                    disabled={busy !== null}
                    onClick={() => run(pack.id, () => api.removePack(pack.id), null)}
                  >
                    Remove
                  </button>
                </div>
              </article>
            ))}
          </div>
        </>
      )}
      <div className="market-footer">
        <button type="button" className="button" onClick={() => api.openPacksFolder()}>
          Open packs folder
        </button>
        <button type="button" className="link-button" onClick={() => api.openLink("submit-pack")}>
          Made a pack? Submit it on GitHub
        </button>
      </div>
    </div>
  );
}
