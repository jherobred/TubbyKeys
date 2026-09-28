import { useCallback, useEffect, useRef, useState } from "react";
import { api, on, type OutputStatus, type PackInfo, type Settings, type Snapshot, type Visualizer } from "./api";

/**
 * Shared app state for the settings window and the tray flyout. Edits apply
 * locally at once and reach the backend at most once per frame, so dragging a
 * slider stays smooth. Backend echoes are ignored while you are mid-edit.
 */
export function useAppState() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const current = useRef<Snapshot | null>(null);
  const scheduled = useRef(false);
  const lastEdit = useRef(0);

  const commit = useCallback((next: Snapshot | null) => {
    current.current = next;
    setSnap(next);
  }, []);

  useEffect(() => {
    api.getState().then(commit, (e) => setError(String(e)));
    const offs = [
      on<Settings>("settings-changed", (settings) => {
        if (!current.current || performance.now() - lastEdit.current < 300) return;
        commit({ ...current.current, settings });
      }),
      on<PackInfo[]>("packs-changed", (packs) => {
        if (current.current) commit({ ...current.current, packs });
      }),
      on<OutputStatus>("output-changed", (output) => {
        if (current.current) commit({ ...current.current, output });
      }),
    ];
    return () => offs.forEach((off) => off());
  }, [commit]);

  const flush = useCallback(() => {
    scheduled.current = false;
    const s = current.current;
    if (!s) return;
    api.updateSettings(s.settings).then(
      () => setError(null),
      (e) => {
        setError(String(e));
        // The backend kept the previous value. Show what is really active.
        api.getState().then(commit);
      },
    );
  }, [commit]);

  const edit = useCallback(
    (settings: Settings) => {
      if (!current.current) return;
      commit({ ...current.current, settings });
      lastEdit.current = performance.now();
      if (!scheduled.current) {
        scheduled.current = true;
        requestAnimationFrame(flush);
      }
    },
    [commit, flush],
  );

  const patch = useCallback(
    (change: Partial<Settings>) => {
      if (current.current) edit({ ...current.current.settings, ...change });
    },
    [edit],
  );

  const patchVisualizer = useCallback(
    (change: Partial<Visualizer>) => {
      const s = current.current?.settings;
      if (s) edit({ ...s, visualizer: { ...s.visualizer, ...change } });
    },
    [edit],
  );

  const setPacks = useCallback(
    (packs: PackInfo[]) => {
      if (current.current) commit({ ...current.current, packs });
    },
    [commit],
  );

  return { snap, error, setError, patch, patchVisualizer, setPacks };
}
