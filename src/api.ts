import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Style = "keyboard" | "pill" | "wave";
export type Size = "small" | "medium" | "large";
export type Placement = "top" | "bottom" | "random" | "custom";
export type Animation = "pop" | "slide" | "bounce" | "pulse";
export type Idle = "hide" | "fade" | "keep";

export interface Visualizer {
  enabled: boolean;
  style: Style;
  size: Size;
  showCombo: boolean;
  placement: Placement;
  /** Where it was dragged to, in screen pixels. */
  position: [number, number] | null;
  animation: Animation;
  /** Milliseconds without a key before the combo resets. 0 keeps it forever. */
  comboTimeoutMs: number;
  idle: Idle;
  hideInFullscreen: boolean;
  hideFromCapture: boolean;
}

export interface Settings {
  enabled: boolean;
  packId: string;
  volume: number;
  /** -1 thock .. 1 clack */
  tone: number;
  /** -1 deep .. 1 sharp */
  pitch: number;
  randomizePitch: boolean;
  spatial: boolean;
  stereoWidth: number;
  /** -1 left only .. 0 middle .. 1 right only */
  balance: number;
  headphoneWidth: boolean;
  hotkey: string;
  visualizer: Visualizer;
  trayPulse: boolean;
}

export interface PackInfo {
  id: string;
  name: string;
  version: string;
  author: string;
  license: string;
  type: string;
  description: string;
  source: string | null;
  builtin: boolean;
}

export interface OutputStatus {
  device: string | null;
  headphones: boolean;
}

export interface Snapshot {
  settings: Settings;
  packs: PackInfo[];
  output: OutputStatus;
  version: string;
}

export interface MarketItem {
  id: string;
  name: string;
  version: string;
  author: string;
  license: string;
  type: string;
  description: string;
  size: number;
  installedVersion: string | null;
}

export interface KeyPulse {
  row: number;
  x: number;
  wide: boolean;
}

export type Link = "repo" | "submit-pack" | "issues";

export const api = {
  getState: () => invoke<Snapshot>("get_state"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  previewPack: (id: string) => invoke<void>("preview_pack", { id }),
  marketList: () => invoke<MarketItem[]>("market_list"),
  marketInstall: (id: string) => invoke<PackInfo[]>("market_install", { id }),
  removePack: (id: string) => invoke<PackInfo[]>("remove_pack", { id }),
  openPacksFolder: () => invoke<void>("open_packs_folder"),
  openLink: (link: Link) => invoke<void>("open_link", { link }),
  showSettings: () => invoke<void>("show_settings"),
  arrangeOverlay: (active: boolean) => invoke<Settings>("arrange_overlay", { active }),
  quit: () => invoke<void>("quit_app"),
};

/** Subscribe to a backend event. Returns an unsubscribe function. */
export function on<T>(event: string, handler: (payload: T) => void): () => void {
  let stop: (() => void) | undefined;
  let cancelled = false;
  listen<T>(event, (e) => handler(e.payload)).then((unlisten) => {
    if (cancelled) unlisten();
    else stop = unlisten;
  });
  return () => {
    cancelled = true;
    stop?.();
  };
}
