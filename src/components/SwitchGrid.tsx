import { useRef } from "react";
import { api, type PackInfo } from "../api";
import { SwitchIcon } from "./Art";

const GROUPS: { title: string; match: (p: PackInfo) => boolean }[] = [
  { title: "Linear", match: (p) => p.builtin && p.type === "linear" },
  { title: "Tactile", match: (p) => p.builtin && p.type === "tactile" },
  { title: "Clicky", match: (p) => p.builtin && p.type === "clicky" },
  { title: "Community", match: (p) => !p.builtin },
];

/** Switch picker. Hovering a card previews its sound; clicking selects it. */
export function SwitchGrid(props: { packs: PackInfo[]; active: string; onSelect: (id: string) => void }) {
  const hoverTimer = useRef<number | undefined>(undefined);

  const preview = (id: string) => {
    window.clearTimeout(hoverTimer.current);
    // A short delay so sweeping the mouse across the grid stays quiet.
    hoverTimer.current = window.setTimeout(() => {
      api.previewPack(id).catch(() => undefined);
    }, 140);
  };
  const cancel = () => window.clearTimeout(hoverTimer.current);

  const grouped = GROUPS.map((g) => ({ ...g, packs: props.packs.filter(g.match) }));
  const other = props.packs.filter((p) => !GROUPS.some((g) => g.match(p)));
  if (other.length) grouped.push({ title: "Other", match: () => false, packs: other });

  return (
    <div className="switch-groups">
      {grouped
        .filter((g) => g.packs.length > 0)
        .map((group) => (
          <section key={group.title}>
            <h3 className="group-title">{group.title}</h3>
            <div className="switch-grid">
              {group.packs.map((pack) => (
                <button
                  key={pack.id}
                  type="button"
                  className={`switch-card${pack.id === props.active ? " active" : ""}`}
                  aria-pressed={pack.id === props.active}
                  title={pack.description || pack.name}
                  onMouseEnter={() => preview(pack.id)}
                  onMouseLeave={cancel}
                  onFocus={() => preview(pack.id)}
                  onClick={() => {
                    cancel();
                    props.onSelect(pack.id);
                  }}
                >
                  <SwitchIcon id={pack.id} />
                  <span className="switch-name">{pack.name}</span>
                  <span className="switch-meta">{pack.builtin ? pack.type : pack.author || "Community"}</span>
                </button>
              ))}
            </div>
          </section>
        ))}
    </div>
  );
}
