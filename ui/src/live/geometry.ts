// Screen geometry: corner pinning (keystone) and the soft edges of a projector wall.

type P = [number, number];

/** Solves the 8×8 system for the projective map from four points to four points. */
function homography(src: P[], dst: P[]): number[] | null {
  const a: number[][] = [];
  const b: number[] = [];
  for (let i = 0; i < 4; i++) {
    const [x, y] = src[i];
    const [u, v] = dst[i];
    a.push([x, y, 1, 0, 0, 0, -u * x, -u * y]);
    b.push(u);
    a.push([0, 0, 0, x, y, 1, -v * x, -v * y]);
    b.push(v);
  }
  // Gaussian elimination with partial pivoting
  const n = 8;
  for (let c = 0; c < n; c++) {
    let p = c;
    for (let r = c + 1; r < n; r++) if (Math.abs(a[r][c]) > Math.abs(a[p][c])) p = r;
    if (Math.abs(a[p][c]) < 1e-12) return null;
    [a[c], a[p]] = [a[p], a[c]];
    [b[c], b[p]] = [b[p], b[c]];
    for (let r = c + 1; r < n; r++) {
      const f = a[r][c] / a[c][c];
      for (let k = c; k < n; k++) a[r][k] -= f * a[c][k];
      b[r] -= f * b[c];
    }
  }
  const h = new Array(8).fill(0);
  for (let r = n - 1; r >= 0; r--) {
    let s = b[r];
    for (let k = r + 1; k < n; k++) s -= a[r][k] * h[k];
    h[r] = s / a[r][r];
  }
  return [...h, 1];
}

/** Where a point goes under the map (for tests and the settings sketch). */
export function mapPoint(h: number[], [x, y]: P): P {
  const w = h[6] * x + h[7] * y + h[8];
  return [(h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w];
}

/** The corners after pinning, in pixels: pin = [tl x, y, tr x, y, br x, y, bl x, y] in %,
 *  positive inward. */
export function pinnedCorners(pin: number[], w: number, h: number): P[] {
  const [a, b, c, d, e, f, g, k] = pin;
  return [
    [(a / 100) * w, (b / 100) * h],
    [w - (c / 100) * w, (d / 100) * h],
    [w - (e / 100) * w, h - (f / 100) * h],
    [(g / 100) * w, h - (k / 100) * h],
  ];
}

export function pinMatrix(pin: number[], w: number, h: number): number[] | null {
  return homography(
    [
      [0, 0],
      [w, 0],
      [w, h],
      [0, h],
    ],
    pinnedCorners(pin, w, h),
  );
}

/** The CSS transform for a corner pin (with transform-origin 0 0); null: none needed. */
export function pinTransform(pin: number[] | null | undefined, w: number, h: number): string | null {
  if (!pin || pin.every((v) => v === 0) || w <= 0 || h <= 0) return null;
  const m = pinMatrix(pin, w, h);
  if (!m) return null;
  // CSS matrix3d is column-major: [a b 0 g; c d 0 h; 0 0 1 0; e f 0 i] for the 3×3 map
  return `matrix3d(${[m[0], m[3], 0, m[6], m[1], m[4], 0, m[7], 0, 0, 1, 0, m[2], m[5], 0, m[8]].join(",")})`;
}

/** The darkening across a soft edge (0 at the inner side, 1 at the outer edge): the light
 *  that stays falls off linearly in real brightness, so two overlapping projectors add up
 *  evenly; gamma turns that into the values the projector is sent. */
export function blendAlpha(t: number, gamma: number): number {
  const light = 1 - Math.min(1, Math.max(0, t)); // real brightness that stays
  return 1 - Math.pow(light, 1 / gamma);
}

/** A CSS gradient for one soft edge, from the inner side (clear) to the outer edge. */
export function blendGradient(direction: "left" | "right" | "top" | "bottom", gamma: number): string {
  const stops = Array.from({ length: 11 }, (_, i) => `rgba(0,0,0,${blendAlpha(i / 10, gamma).toFixed(3)}) ${i * 10}%`);
  const toward = { left: "to left", right: "to right", top: "to top", bottom: "to bottom" }[direction];
  return `linear-gradient(${toward}, ${stops.join(", ")})`;
}
