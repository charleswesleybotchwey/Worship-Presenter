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
  /** EasyWorship's "Capitalise all words" / "first word of each line" (as drawn only). */
  capitalize?: "none" | "words" | "lines";
  /** Word wrapping (default on); off keeps each line whole and shrinks to fit the width. */
  wrap?: boolean;
  /** Margins inside the text box, in % of the screen height. */
  margin?: { l: number; r: number; t: number; b: number };
  /** The same size on every slide of the item (the size at which all of them fit). */
  normalize?: boolean;
  /** Even line lengths (no long line and a short leftover word). */
  balance?: boolean;
  /** How the words appear on each new slide (EasyWorship's text animations). */
  enter?: "none" | "fade" | "rise" | "fly" | "type";
  autoFit: boolean;
  minSize: number;
  shadow: { on: boolean; color: string; blur: number; x: number; y: number };
  outline: { on: boolean; color: string; width: number };
  /** Words filled with a gradient instead of one colour. */
  gradient?: Gradient | null;
  /** ProPresenter-style box behind the words that grows with them. "block": one box
   *  around all the text; "lines": a strip behind each line. Sizes are % of height. */
  box?: TextBox;
}

export interface TextBox {
  mode: "off" | "block" | "lines";
  color: string;
  padX: number;
  padY: number;
  radius: number;
  /** Glide to the new size when the words change. */
  animate: boolean;
  /** Frosted glass: blur what is behind the box (% of height; 0 = off). */
  blur?: number;
}

export const BOX_DEFAULT: TextBox = { mode: "off", color: "#000000d9", padX: 2, padY: 1, radius: 0.6, animate: true };

interface LayerBase extends Rect {
  id: string;
  name: string;
  opacity: number;
  hidden: boolean;
  locked: boolean;
  /** Show this layer on every slide (default), or only on an item's first or last slide. */
  onlyOn?: "first" | "last" | null;
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

/** A two-colour gradient; angle in degrees (0 = upwards, 90 = to the right, as CSS). */
export interface Gradient {
  from: string;
  to: string;
  angle: number;
}
export const gradientCss = (g: Gradient) => `linear-gradient(${g.angle}deg, ${g.from}, ${g.to})`;

export interface ShapeLayer extends LayerBase {
  kind: "shape";
  shape: "rect";
  fill: string;
  /** Overrides the fill colour when set. */
  gradient?: Gradient | null;
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
  background: {
    kind: "color" | "image" | "video" | "transparent" | "gradient";
    color: string;
    image: string | null;
    video?: string | null;
    poster?: string | null;
    /** cover: fill the screen; contain: show all of it; fill: stretch to the screen */
    fit: "cover" | "contain" | "fill";
    dim: number;
    /** EasyWorship's background Rotate (degrees) and flips, opacity and video volume. */
    turn?: number;
    flipH?: boolean;
    flipV?: boolean;
    /** 0–1 (1 = as it is) */
    opacity?: number;
    /** 0–1; heard from the main output only (0 = silent, the default) */
    volume?: number;
    gradient?: Gradient | null;
    /** More moving backgrounds that take turns with the main one. */
    set?: { video: string; poster: string | null }[];
    /** When the turn passes: each song/reading gets its own, or one per service (day). */
    rotate?: "item" | "service";
  };
  /** Two languages line by line (each line, then its translation) in the words layer. */
  pairLines?: boolean;
  /** When only the second language is shown (Live: Fante only), it takes the main text's
   *  place and size. Off: it keeps the second-language layer's look. Default on. */
  secondFills?: boolean;
  /** Songs: the title slide's pattern ("{number} · {title}\n{author}"); empty = that. */
  titleSlide?: string;
  transition: { kind: "cut" | "fade" | "slide-up" | "wipe" | "zoom" | "push" | "cover" | "cube" | "drop" | "iris" | "pageflip"; ms: number };
  layers: Layer[];
  /** Choruses, bridges… styled apart from the verses. */
  sections?: Partial<Record<SectionKind, SectionStyle>>;
}

/** How a chorus, bridge… looks different from the verses (song themes). */
export interface SectionStyle {
  italic?: boolean;
  color?: string | null;
  weight?: number | null;
}
export type SectionKind = "chorus" | "bridge" | "pre-chorus" | "tag" | "ending";

/** "Chorus 2" → chorus, "Pre-chorus" → pre-chorus; verses have no section style. */
export function sectionKindOf(label: string | null | undefined): SectionKind | null {
  const l = (label ?? "").toLowerCase();
  if (l.startsWith("pre-chorus")) return "pre-chorus";
  for (const k of ["chorus", "bridge", "tag", "ending"] as const) if (l.startsWith(k)) return k;
  return null;
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
  /** This view may play a background video's sound (the main output only). */
  sound?: boolean;
  primary: string[];
  second: string[];
  /** Every slide's words in this item (for "the same size on every slide"). */
  allPrimary?: string[][];
  allSecond?: string[][];
  /** "Verse 1", "Chorus"… for songs (shown only by a section-label layer). */
  section?: string | null;
  /** The slide is its item's first / last (for layers shown only there). */
  first?: boolean;
  last?: boolean;
  /** The operator's quick change for this item: words larger (×) and moved (% of height). */
  textScale?: number;
  textShift?: number;
  /** Picks the moving background from a set: the item's title (per song/reading). */
  rotateKey?: string | null;
  reference?: string | null;
  title?: string | null;
  copyright?: string | null;
}
