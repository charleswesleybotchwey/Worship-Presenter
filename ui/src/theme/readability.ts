import type { Theme } from "./types";

// Is the text readable? For each text layer, estimate the colour behind the words (its
// box, the shapes under it, the background colour, or the average of the background
// picture under the layer, after darkening) and compare it with the words' colour.

export interface ReadIssue {
  layerId: string;
  name: string;
  /** WCAG contrast ratio, 1 (none) to 21 (black on white). */
  ratio: number;
  /** What fixes it: darken the picture, or put a box behind the words. */
  fix: "dim" | "box";
}

type RGB = [number, number, number];

function parse(c: string): { rgb: RGB; a: number } | null {
  const h = c.trim().replace("#", "");
  if (!/^[0-9a-f]{3,8}$/i.test(h)) return null;
  const full = h.length <= 4 ? [...h].map((x) => x + x).join("") : h;
  const rgb = [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16)) as RGB;
  const a = full.length >= 8 ? parseInt(full.slice(6, 8), 16) / 255 : 1;
  return { rgb, a };
}
const over = (top: RGB, a: number, under: RGB): RGB => top.map((v, i) => v * a + under[i] * (1 - a)) as RGB;
const lin = (v: number) => {
  const s = v / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};
const luminance = (c: RGB) => 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
export const contrast = (a: RGB, b: RGB) => {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

/** Average colour of a picture inside a rectangle (in % of the frame), or null. */
async function averageOf(src: string, rect: { x: number; y: number; w: number; h: number }, fit: "cover" | "contain" | "fill" | "fill"): Promise<RGB | null> {
  try {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.src = src;
    await img.decode();
    const W = 160;
    const H = 90;
    const c = document.createElement("canvas");
    c.width = W;
    c.height = H;
    const g = c.getContext("2d", { willReadFrequently: true });
    if (!g) return null;
    const scale = fit === "cover" ? Math.max(W / img.naturalWidth, H / img.naturalHeight) : Math.min(W / img.naturalWidth, H / img.naturalHeight);
    const dw = img.naturalWidth * scale;
    const dh = img.naturalHeight * scale;
    g.fillStyle = "#000";
    g.fillRect(0, 0, W, H);
    g.drawImage(img, (W - dw) / 2, (H - dh) / 2, dw, dh);
    const x = Math.max(0, Math.floor((rect.x / 100) * W));
    const y = Math.max(0, Math.floor((rect.y / 100) * H));
    const w = Math.max(1, Math.min(W - x, Math.ceil((rect.w / 100) * W)));
    const h = Math.max(1, Math.min(H - y, Math.ceil((rect.h / 100) * H)));
    const d = g.getImageData(x, y, w, h).data; // throws if the picture is from another origin
    let r = 0, gg = 0, b = 0;
    for (let i = 0; i < d.length; i += 4) (r += d[i]), (gg += d[i + 1]), (b += d[i + 2]);
    const n = d.length / 4;
    return [r / n, gg / n, b / n];
  } catch {
    return null;
  }
}

export async function checkReadability(theme: Theme, url: (s: string) => string): Promise<ReadIssue[]> {
  const bg = theme.background;
  const issues: ReadIssue[] = [];
  for (const [i, l] of theme.layers.entries()) {
    if (l.kind !== "text" || l.hidden) continue;
    const words = parse(l.style.color);
    if (!words) continue;
    // what is behind: start with the background
    let behind: RGB | null = null;
    let fromPicture = false;
    if (bg.kind === "color") behind = parse(bg.color)?.rgb ?? null;
    else if (bg.kind === "gradient" && bg.gradient) {
      // a gradient reads as roughly the middle of its two colours
      const a = parse(bg.gradient.from)?.rgb;
      const b = parse(bg.gradient.to)?.rgb;
      behind = a && b ? (a.map((v, k) => (v + b[k]) / 2) as RGB) : null;
    }
    else if (bg.kind === "image" && bg.image) (behind = await averageOf(url(bg.image), l, bg.fit)), (fromPicture = true);
    else if (bg.kind === "video" && bg.poster) (behind = await averageOf(url(bg.poster), l, bg.fit)), (fromPicture = true);
    else if (bg.kind === "transparent") continue; // keyed over the stream: unknown
    if (!behind) continue;
    if (fromPicture && bg.dim > 0) behind = over([0, 0, 0], bg.dim, behind);
    // shapes under the layer that cover its middle
    const cx = l.x + l.w / 2;
    const cy = l.y + l.h / 2;
    let shaped = false;
    for (const s of theme.layers.slice(0, i)) {
      if (s.kind !== "shape" || s.hidden) continue;
      if (cx >= s.x && cx <= s.x + s.w && cy >= s.y && cy <= s.y + s.h) {
        const f = parse(s.fill);
        if (f && f.a > 0) (behind = over(f.rgb, f.a * s.opacity, behind)), (shaped = true);
      }
    }
    // a box behind the words is what they are read against
    if (l.style.box && l.style.box.mode !== "off") {
      const f = parse(l.style.box.color);
      if (f) (behind = over(f.rgb, f.a, behind)), (shaped = true);
    }
    const ratio = contrast(over(words.rgb, words.a, behind), behind);
    // big screen text: 3:1 is the least that stays readable from the back of a hall
    if (ratio < 3) issues.push({ layerId: l.id, name: l.name, ratio, fix: fromPicture && !shaped ? "dim" : "box" });
  }
  return issues;
}

/** Text layers that reach outside the title-safe area (10 % in from each edge). */
export function outsideSafe(theme: Theme): string[] {
  return theme.layers.filter((l) => l.kind === "text" && !l.hidden && (l.x < 5 || l.y < 5 || l.x + l.w > 95 || l.y + l.h > 95)).map((l) => l.name);
}
