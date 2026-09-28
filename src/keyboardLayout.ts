/**
 * Keys drawn by the visualizer, in the same units as src-tauri/src/keymap.rs:
 * `x` is the key centre in key widths, `w` its width. A pulse from the backend
 * lights the key in its row whose centre is nearest.
 */
export interface KeyRect {
  row: number;
  x: number;
  w: number;
}

export const BOARD_WIDTH = 22.5;

const run = (start: number, count: number): [number, number][] =>
  Array.from({ length: count }, (_, i) => [start + i, 1]);

const ROWS: [number, number][][] = [
  [[0.5, 1], ...run(1.5, 12), [14, 2], ...run(15.5, 3), ...run(18.5, 4)],
  [[0.75, 1.5], ...run(2, 12), [14.25, 1.5], ...run(15.5, 3), ...run(18.5, 4)],
  [[0.9, 1.75], ...run(2.25, 11), [13.9, 2.25], ...run(18.5, 3)],
  [[1.1, 2.25], ...run(2.75, 10), [13.6, 2.75], [16.5, 1], ...run(18.5, 4)],
  [[0.6, 1.25], [1.9, 1.25], [3.1, 1.25], [7, 6.25], [10.9, 1.25], [12.1, 1.25], [13.3, 1.25], [14.4, 1.25], ...run(15.5, 3), [19, 2], [20.5, 1]],
];

export const KEYS: KeyRect[] = ROWS.flatMap((keys, row) => keys.map(([x, w]) => ({ row, x, w })));

/** Index into KEYS of the key nearest to a pulse. */
export function nearestKey(row: number, x: number): number {
  const target = x * BOARD_WIDTH;
  let best = -1;
  let bestDistance = Infinity;
  KEYS.forEach((key, i) => {
    if (key.row !== row) return;
    const distance = Math.abs(key.x - target);
    if (distance < bestDistance) {
      best = i;
      bestDistance = distance;
    }
  });
  return best;
}
