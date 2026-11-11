import { useEffect, useState } from "react";
import { api, onSettingsChanged } from "../api";
import type { Theme } from "./types";

// The church's brand kit: four colours and two fonts, set once (Settings → Brand kit).
// A theme can refer to them ("brand:2", "brand:2/80" with opacity; font "brand" or
// "brand-heading"), so changing the brand changes every theme that uses it.

export interface Brand {
  colors: string[];
  font: string;
  heading: string;
}
export const BRAND_NAMES = ["Main", "Accent", "Text", "Background"];
export const BRAND_DEFAULT: Brand = { colors: ["#1f4e9c", "#f2b705", "#ffffff", "#0d1424"], font: "Noto Sans", heading: "Noto Sans" };

export function parseBrand(x: Record<string, string>): Brand {
  let colors = BRAND_DEFAULT.colors;
  try {
    const c = JSON.parse(x.brand_colors || "[]");
    if (Array.isArray(c) && c.length) colors = BRAND_DEFAULT.colors.map((d, i) => (typeof c[i] === "string" && /^#[0-9a-f]{6}$/i.test(c[i]) ? c[i] : d));
  } catch {
    /* keep the defaults */
  }
  return { colors, font: x.brand_font || BRAND_DEFAULT.font, heading: x.brand_heading_font || x.brand_font || BRAND_DEFAULT.heading };
}

/** "brand:2" → the accent colour; "brand:2/80" → with that opacity (hex). */
export function brandColor(v: string, b: Brand): string {
  const m = /^brand:(\d)(?:\/([0-9a-f]{2}))?$/i.exec(v.trim());
  if (!m) return v;
  const c = b.colors[Number(m[1]) - 1] ?? "#ffffff";
  return m[2] && m[2].toLowerCase() !== "ff" ? c + m[2] : c;
}
export const isBrand = (v: string | null | undefined) => !!v && /^brand:\d/.test(v);

/** The theme with brand references turned into real colours and fonts (for drawing). */
export function resolveBrand(t: Theme, b: Brand): Theme {
  const walk = (x: unknown, key = ""): unknown => {
    if (typeof x === "string") {
      if (key === "font") return x === "brand" ? b.font : x === "brand-heading" ? b.heading : x;
      return x.startsWith("brand:") ? brandColor(x, b) : x;
    }
    if (Array.isArray(x)) return x.map((v) => walk(v));
    if (x && typeof x === "object") return Object.fromEntries(Object.entries(x).map(([k, v]) => [k, walk(v, k)]));
    return x;
  };
  return JSON.stringify(t).includes("brand") ? (walk(t) as Theme) : t;
}

let cache: Brand | null = null;
const listeners = new Set<(b: Brand) => void>();
let started = false;
function load() {
  api
    .getSettings()
    .then((x) => {
      cache = parseBrand(x);
      listeners.forEach((f) => f(cache!));
    })
    .catch(() => {});
}
/** The current brand kit (shared by every theme on screen). */
export function useBrand(): Brand {
  const [b, setB] = useState<Brand>(cache ?? BRAND_DEFAULT);
  useEffect(() => {
    listeners.add(setB);
    if (!started) {
      started = true;
      load();
      void onSettingsChanged(load);
    } else if (cache) setB(cache);
    return () => void listeners.delete(setB);
  }, []);
  return b;
}

const lum = (hex: string) => {
  const h = hex.replace("#", "").slice(0, 6);
  const [r, g, bl] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
};
const alphaOf = (c: string) => (/^#[0-9a-f]{8}$/i.test(c) ? c.slice(7, 9).toLowerCase() : "");
const tok = (n: number, c: string) => `brand:${n}${alphaOf(c) && alphaOf(c) !== "ff" ? "/" + alphaOf(c) : ""}`;

/** One click: a theme (often an imported one) re-coloured with the brand kit. Words
 *  take the brand text colour (choruses the accent), thin lines the accent, dark areas
 *  the brand background and the rest the main colour; fonts become the brand fonts. */
export function recolorToBrand(t: Theme): Theme {
  const n = structuredClone(t);
  const area = (c: string, thin: boolean) => {
    if (!/^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(c) || alphaOf(c) === "00") return c;
    if (thin) return tok(2, c);
    return tok(lum(c) < 0.25 ? 4 : 1, c);
  };
  if (n.background.kind === "color") n.background.color = area(n.background.color, false);
  for (const l of n.layers) {
    if (l.kind === "shape") {
      const thin = l.w < 2 || l.h < 2;
      l.fill = area(l.fill, thin);
      if (l.border?.width) l.border.color = tok(2, l.border.color);
    }
    if (l.kind === "text") {
      l.style.color = l.field === "second" ? tok(2, l.style.color) : tok(3, l.style.color);
      l.style.font = l.field === "title" || l.field === "section" ? "brand-heading" : "brand";
      if (l.style.box && l.style.box.mode !== "off") l.style.box.color = area(l.style.box.color, false);
    }
  }
  if (n.sections?.chorus?.color) n.sections.chorus.color = "brand:2";
  return n;
}
