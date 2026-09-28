import { useId } from "react";

/** The TubbyKeys mascot: a chubby keycap. Same drawing as the app icon. */
export function Mascot({ size = 32 }: { size?: number }) {
  const id = useId();
  return (
    <svg viewBox="0 0 1024 1024" width={size} height={size} aria-hidden="true">
      <defs>
        <linearGradient id={`${id}s`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#34C6A3" />
          <stop offset="1" stopColor="#1B917A" />
        </linearGradient>
        <linearGradient id={`${id}f`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#A6F5DF" />
          <stop offset="1" stopColor="#62DDBD" />
        </linearGradient>
      </defs>
      <rect x="84" y="150" width="856" height="790" rx="250" fill={`url(#${id}s)`} />
      <rect x="170" y="186" width="684" height="612" rx="200" fill={`url(#${id}f)`} />
      <rect x="250" y="226" width="400" height="64" rx="32" fill="#fff" opacity="0.4" />
      <ellipse cx="398" cy="478" rx="44" ry="58" fill="#12302A" />
      <ellipse cx="626" cy="478" rx="44" ry="58" fill="#12302A" />
      <circle cx="413" cy="457" r="14" fill="#fff" />
      <circle cx="641" cy="457" r="14" fill="#fff" />
      <ellipse cx="300" cy="575" rx="62" ry="36" fill="#FF8FA3" opacity="0.8" />
      <ellipse cx="724" cy="575" rx="62" ry="36" fill="#FF8FA3" opacity="0.8" />
      <path d="M458 572 Q512 628 566 572" stroke="#12302A" strokeWidth="28" strokeLinecap="round" fill="none" />
    </svg>
  );
}

/** Stem colours of the built-in switches. Community packs get a colour from their id. */
const STEM_COLORS: Record<string, string> = {
  cream: "#EFE3C2",
  "holy-panda": "#F4F4F4",
  alpaca: "#F4B6C8",
  "turquoise-tealios": "#39C6BE",
  "black-ink": "#34373C",
  "red-ink": "#E0454F",
  "mx-black": "#2A2A2A",
  "mx-brown": "#8B5A2B",
  "mx-blue": "#2F6FE0",
  "box-navy": "#23407A",
  "buckling-spring": "#A7B0BA",
  "blue-alps": "#4F86C6",
  topre: "#C7C9CC",
};

export function stemColor(id: string): string {
  if (STEM_COLORS[id]) return STEM_COLORS[id];
  let hash = 0;
  for (const ch of id) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 60% 58%)`;
}

/** A switch seen from above: housing plus a cross stem in the switch colour. */
export function SwitchIcon({ id, size = 44 }: { id: string; size?: number }) {
  return (
    <svg viewBox="0 0 48 48" width={size} height={size} aria-hidden="true" className="switch-icon">
      <rect x="3" y="3" width="42" height="42" rx="10" className="switch-housing" />
      <rect x="10" y="10" width="28" height="28" rx="7" className="switch-top" />
      <path
        d="M21 13.5h6v7.5h7.5v6H27v7.5h-6V27h-7.5v-6H21z"
        fill={stemColor(id)}
        className="switch-stem"
        strokeWidth="1.2"
      />
    </svg>
  );
}
