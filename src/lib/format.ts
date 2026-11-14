// Display formatting for data values.

/** Coverage ratio 0..1 as a percentage with one decimal: 0.8231 → "82.3%". */
export function formatPercent(ratio: number): string {
  return `${(ratio * 100).toFixed(1)}%`;
}

/** Integer with thousands separators: 239796 → "239,796". */
export function formatInt(value: number): string {
  return Math.round(value).toLocaleString('en-US');
}

/** Elapsed milliseconds as seconds with one decimal: 1840 → "1.8 s". */
export function formatSeconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)} s`;
}

/** Last path segment of a Windows or POSIX path. */
export function fileName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/** Canvas size: [2048, 2048] → "2048×2048". */
export function formatCanvas(canvas: readonly [number, number]): string {
  return `${canvas[0]}×${canvas[1]}`;
}

/** Customer ids are digits only. */
export function isCustomerId(value: string): boolean {
  return /^\d{0,12}$/.test(value);
}
