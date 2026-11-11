// The theme model. Positions and sizes are percentages of the canvas; font sizes and
// effects are percentages of the canvas height, so one theme works on any screen.

import classic from "../../../crates/wp-core/themes/classic.json";
import scripture from "../../../crates/wp-core/themes/scripture.json";
import lowerThird from "../../../crates/wp-core/themes/lower-third.json";
import announcement from "../../../crates/wp-core/themes/announcement.json";

export type ThemeKind = "song" | "scripture" | "announcement" | "lower_third" | "foldback";
export type AnimKind = "none" | "fade" | "slide-up" | "slide-down" | "slide-left" | "slide-right" | "wipe" | "scale";
export type Field = "lyrics" | "second" | "reference" | "title" | "copyright" | "clock" | "free" | "section";

export interface Anim {
  kind: AnimKind;
  ms: number;
  delay: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface TextStyle {
  font: string;
  size: number;
  weight: number;
  italic: boolean;
  color: string;
  align: "left" | "center" | "right";
  valign: "top" | "middle" | "bottom";
  lineHeight: number;
  letterSpacing: number;
  uppercase: boolean;
  autoFit: boolean;
  minSize: number;
  shadow: { on: boolean; color: string; blur: number; x: number; y: number };
  outline: { on: boolean; color: string; width: number };
}

interface LayerBase extends Rect {
  id: string;
  name: string;
  opacity: number;
  hidden: boolean;
  locked: boolean;
  animIn: Anim;
  animOut: Anim;
}

export interface TextLayer extends LayerBase {
  kind: "text";
  field: Field;
  text: string;
  /** Box to use when the second-language/second-version text is empty. */
  solo: Rect | null;
  style: TextStyle;
}

export interface ShapeLayer extends LayerBase {
  kind: "shape";
  shape: "rect";
  fill: string;
  radius: number;
  border: { color: string; width: number };
}

export interface ImageLayer extends LayerBase {
  kind: "image";
  src: string;
  fit: "contain" | "cover";
}

export interface TickerLayer extends LayerBase {
  kind: "ticker";
  text: string;
  speed: number;
  style: TextStyle;
  fill: string;
}

export type Layer = TextLayer | ShapeLayer | ImageLayer | TickerLayer;

export interface Theme {
  schema_version: number;
  name: string;
  kind: ThemeKind;
  /** image/video/poster: a data: URL, or "media:<path>" for a file in the media library. */
  background: { kind: "color" | "image" | "video" | "transparent"; color: string; image: string | null; video?: string | null; poster?: string | null; fit: "cover" | "contain"; dim: number };
  transition: { kind: "cut" | "fade"; ms: number };
  layers: Layer[];
}

export interface ThemeRow {
  id: number;
  name: string;
  kind: ThemeKind;
  is_default: boolean;
  theme: Theme;
}

export interface ActiveThemes {
  rev: number;
  song: Theme;
  scripture: Theme;
  lower_third: Theme;
  announcement: Theme;
}

export const BUILTIN = { song: classic as Theme, scripture: scripture as Theme, lower_third: lowerThird as Theme, announcement: announcement as Theme };

export const KIND_NAMES: Record<ThemeKind, string> = {
  song: "Songs",
  scripture: "Scripture",
  lower_third: "Lower third",
  announcement: "Announcements",
  foldback: "Stage display",
};

export const FIELD_NAMES: Record<Field, string> = {
  lyrics: "Lyrics / verse text",
  second: "Second language / version",
  reference: "Scripture reference",
  title: "Song title",
  copyright: "Copyright line",
  clock: "Clock",
  free: "Fixed text",
  section: "Section label (Verse 1, Chorus)",
};

// Noto Sans and Inter ship with the app. The rest are on most Windows PCs.
// Every theme falls back to Noto Sans, so ɛ ɔ ŋ never show as boxes.
export const FONTS = ["Noto Sans", "Inter", "Segoe UI", "Arial", "Calibri", "Cambria", "Georgia", "Times New Roman", "Trebuchet MS", "Verdana", "Tahoma", "Bahnschrift", "Impact"];

// EasyWorship's font fallback list: fonts tried, in order, for any letter the theme's
// font does not have (Noto Sans covers ɛ ɔ ŋ and the tone marks).
const DEFAULT_FALLBACK = ["Noto Sans", "Segoe UI"];
let fallback = DEFAULT_FALLBACK;
export function setFontFallback(list: string[]) {
  const clean = list.map((f) => f.trim()).filter(Boolean);
  fallback = clean.length ? clean : DEFAULT_FALLBACK;
}
export const fontStack = (f: string) => [f, ...fallback.filter((x) => x !== f)].map((x) => `"${x.replace(/"/g, "")}"`).join(", ") + ", system-ui, sans-serif";

const baseStyle = (): TextStyle => structuredClone((classic as Theme).layers[0] as TextLayer).style;
const anim = (kind: AnimKind = "fade", ms = 300, delay = 0): Anim => ({ kind, ms, delay });

let n = 0;
export const newId = (prefix: string) => `${prefix}-${Date.now().toString(36)}${(n++).toString(36)}`;

export function newLayer(kind: Layer["kind"]): Layer {
  const base = { name: "", opacity: 1, hidden: false, locked: false, animIn: anim(), animOut: anim("fade", 200) };
  switch (kind) {
    case "text":
      return { ...base, id: newId("text"), kind, name: "Text", field: "free", text: "Your text", solo: null, x: 20, y: 40, w: 60, h: 20, style: { ...baseStyle(), size: 6 } };
    case "shape":
      return { ...base, id: newId("shape"), kind, name: "Shape", shape: "rect", fill: "#ffffff33", radius: 0, border: { color: "#ffffff00", width: 0 }, x: 10, y: 70, w: 80, h: 20 };
    case "image":
      return { ...base, id: newId("image"), kind, name: "Image", src: "", fit: "contain", x: 80, y: 6, w: 14, h: 14 };
    case "ticker":
      return { ...base, id: newId("ticker"), kind, name: "Ticker", text: "Welcome to our service  •  Offering after the sermon", speed: 12, fill: "#000000aa", style: { ...baseStyle(), size: 3.5, weight: 500, autoFit: false, align: "left" }, x: 0, y: 92, w: 100, h: 8, animIn: anim("slide-up") };
  }
}

export function blankTheme(kind: ThemeKind): Theme {
  const src = kind === "lower_third" ? BUILTIN.lower_third : kind === "scripture" ? BUILTIN.scripture : BUILTIN.song;
  return { ...structuredClone(src), name: `New ${KIND_NAMES[kind].toLowerCase()} theme`, kind };
}

/** Content a theme draws: the current slide plus item details. */
export interface SlideData {
  primary: string[];
  second: string[];
  /** "Verse 1", "Chorus"… for songs (shown only by a section-label layer). */
  section?: string | null;
  reference?: string | null;
  title?: string | null;
  copyright?: string | null;
}
