import { renderLine, stripRich } from "../rich";
import { Crossfade, useMediaFadeMs } from "../fade";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { resolveBrand, useBrand } from "./brand";
import { useMediaUrl } from "./media";
import { fontStack, gradientCss, sectionKindOf, type Layer, type TextBox, type SlideData, type TextLayer, type TextStyle, type Theme, type TickerLayer } from "./types";

// Draws a theme with real content. Used by the theme editor, thumbnails, Preview,
// Live and the output windows, so what is designed is exactly what projects.
//
// phase "in" plays each layer's in-animation (when something goes on air),
// "out" plays the out-animation (going blank), "static" draws without motion.
// `textKey` changes on every slide change; text layers then cross-fade with the
// theme's transition while bands and images stay put.

export type Phase = "in" | "out" | "static";

/** Marks a translation line inside the words layer (two languages line by line). */
export const PAIRED = "\u2063";

/** Each line, then its translation (the extra lines of the longer one at the end). */
export function pairLines(primary: string[], second: string[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < Math.max(primary.length, second.length); i++) {
    if (i < primary.length) out.push(primary[i]);
    if (i < second.length) out.push(PAIRED + second[i]);
  }
  return out;
}

function contentFor(l: TextLayer, d: SlideData, clock: string, paired = false): string[] {
  switch (l.field) {
    case "lyrics":
      return paired && d.second.length ? pairLines(d.primary, d.second) : d.primary;
    case "second":
      return paired ? [] : d.second;
    case "reference":
      return d.reference ? [d.reference] : [];
    case "title":
      return d.title ? [d.title] : [];
    case "copyright":
      return d.copyright ? [d.copyright] : [];
    case "clock":
      return [clock];
    case "free":
      return l.text ? l.text.split("\n") : [];
    case "section":
      return d.section ? [d.section] : [];
  }
}

function textCss(s: TextStyle, sizeCqh: number): CSSProperties {
  return {
    fontFamily: fontStack(s.font),
    fontSize: `${sizeCqh}cqh`,
    fontWeight: s.weight,
    fontStyle: s.italic ? "italic" : "normal",
    color: s.color,
    textAlign: s.align,
    lineHeight: s.lineHeight,
    letterSpacing: `${s.letterSpacing}em`,
    textTransform: s.uppercase ? "uppercase" : "none",
    whiteSpace: s.wrap === false ? "nowrap" : undefined,
    ...(s.balance ? ({ textWrap: "balance" } as CSSProperties) : {}),
    textShadow: s.shadow.on ? `${s.shadow.x}cqh ${s.shadow.y}cqh ${s.shadow.blur}cqh ${s.shadow.color}` : "none",
    WebkitTextStroke: s.outline.on ? `${s.outline.width}cqh ${s.outline.color}` : undefined,
    paintOrder: "stroke fill",
    // gradient words: the gradient shows through the letters
    ...(s.gradient ? { backgroundImage: gradientCss(s.gradient), WebkitBackgroundClip: "text", backgroundClip: "text", color: "transparent" } : null),
  };
}

/** Which moving background plays: the main one or one from the set, taking turns per
 *  song/reading (by its title) or per service (by the date), always the same for both. */
function pickVideo(bg: Theme["background"], key: string | null | undefined): { video: string | null | undefined; poster: string | null | undefined } {
  const all = [{ video: bg.video, poster: bg.poster }, ...(bg.set ?? [])].filter((v) => v.video);
  if (all.length < 2) return { video: bg.video, poster: bg.poster };
  const seed = bg.rotate === "service" ? new Date().toDateString() : key ?? "";
  let h = 0;
  for (const ch of seed) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return all[h % all.length];
}

function animCss(l: Layer, phase: Phase): { className: string; style: CSSProperties } {
  const a = phase === "out" ? l.animOut : l.animIn;
  if (phase === "static" || a.kind === "none") return { className: "", style: {} };
  return {
    className: `anim-${a.kind} ${phase === "out" ? "anim-out" : "anim-in"}`,
    style: { animationDuration: `${a.ms}ms`, animationDelay: `${a.delay}ms` } as CSSProperties,
  };
}

/** Text that shrinks (down to minSize) until it fits its box. */
/** A looping background video; its sound only where allowed (the main output). */
function BgVideo({ src, poster, style, volume }: { src: string; poster?: string; style: CSSProperties; volume: number }) {
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.volume = Math.min(1, Math.max(0, volume));
  }, [volume]);
  return <video ref={ref} className="tv__bg" src={src} poster={poster} style={style} autoPlay muted={volume <= 0} loop playsInline />;
}

/** EasyWorship's capitalisation options, as the words are drawn (the song is unchanged). */
export function capitalise(lines: string[], how: TextStyle["capitalize"]): string[] {
  const up = (w: string) => (w ? w[0].toLocaleUpperCase() + w.slice(1) : w);
  if (how === "words") return lines.map((l) => l.split(/(\s+)/).map(up).join(""));
  if (how === "lines") return lines.map((l) => up(l));
  return lines;
}

/** The size at which every slide's words fit (cached per item and box size). */
const normalCache = new Map<string, number>();

function FitText({ lines: raw, style, valign, others }: { lines: string[]; style: TextStyle; valign: TextStyle["valign"]; others?: string[][] }) {
  const box = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState(style.size);
  const lines = capitalise(raw, style.capitalize);
  const key = lines.join("\n");

  useLayoutEffect(() => {
    const b = box.current;
    const t = inner.current;
    if (!b || !t) return;
    const fit = () => {
      const fits = (sz: number) => {
        t.style.fontSize = `${sz}cqh`;
        return t.scrollHeight <= b.clientHeight + 1 && t.scrollWidth <= b.clientWidth + 1;
      };
      const done = (sz: number) => {
        // always leave the text at the size chosen (a same-size state change draws nothing)
        t.style.fontSize = `${sz}cqh`;
        setSize(sz);
      };
      // "the same size on every slide": the smallest size at which any slide fits
      if (style.normalize && others && others.length > 1) {
        const ck = `${b.clientWidth}x${b.clientHeight}|${JSON.stringify(style)}|${others.map((o) => o.join("\n")).join("\u0001")}`;
        let n = normalCache.get(ck);
        if (n == null) {
          const saved = t.innerHTML;
          let least = style.size;
          for (const o of others) {
            t.innerHTML = capitalise(o, style.capitalize).map((x) => `<p>${stripRich(x).replace(/&/g, "&amp;").replace(/</g, "&lt;")}</p>`).join("");
            if (fits(least)) continue;
            let [lo, hi] = [style.size * 0.25, least];
            for (let i = 0; i < 10; i++) {
              const mid = (lo + hi) / 2;
              if (fits(mid)) lo = mid;
              else hi = mid;
            }
            least = lo;
          }
          t.innerHTML = saved;
          n = least;
          if (normalCache.size > 200) normalCache.clear();
          normalCache.set(ck, n);
        }
        if (fits(n)) return done(n);
      }
      if (fits(style.size)) return done(style.size);
      // words are never cut off: shrink to the theme's smallest size, and below it
      // (down to a quarter) only if they still would not fit; with auto-fit off, the
      // theme's size is kept whenever the words fit at it
      const floor = style.minSize > 0 && style.minSize < style.size ? style.minSize : style.size * 0.6;
      const search = (lo: number, hi: number) => {
        for (let i = 0; i < 10; i++) {
          const mid = (lo + hi) / 2;
          if (fits(mid)) lo = mid;
          else hi = mid;
        }
        return lo;
      };
      if (fits(floor)) return done(search(floor, style.size));
      done(search(style.size * 0.25, floor));
    };
    fit();
    // re-measure when the box changes, when the text's own size changes, and when
    // web fonts finish loading (the fallback font is shorter than Noto Sans)
    const ro = new ResizeObserver(() => requestAnimationFrame(fit));
    ro.observe(b);
    ro.observe(t);
    const fonts = (document as Document & { fonts?: FontFaceSet }).fonts;
    fonts?.addEventListener?.("loadingdone", fit);
    fonts?.ready.then(fit).catch(() => {});
    return () => (ro.disconnect(), fonts?.removeEventListener?.("loadingdone", fit));
  }, [key, style]);

  const bx = style.box?.mode && style.box.mode !== "off" ? style.box : null;
  // the box: padding comes out of the text area, so shrink-to-fit keeps it inside
  const m = style.margin ?? { l: 0, r: 0, t: 0, b: 0 };
  const pad =
    bx || m.l || m.r || m.t || m.b
      ? { padding: `${(bx?.padY ?? 0) + m.t}cqh ${(bx?.padX ?? 0) + m.r}cqh ${(bx?.padY ?? 0) + m.b}cqh ${(bx?.padX ?? 0) + m.l}cqh` }
      : undefined;
  const lineBg = bx?.mode === "lines" ? { background: bx.color, padding: `${bx.padY * 0.5}cqh ${bx.padX}cqh`, borderRadius: `${bx.radius}cqh`, boxDecorationBreak: "clone" as const, WebkitBoxDecorationBreak: "clone" as const, ...(bx.blur ? { backdropFilter: `blur(${bx.blur}cqh)` } : null) } : null;
  return (
    <div ref={box} className={`tl-box tl-box--${valign}`} style={pad}>
      <div ref={inner} className={bx?.mode === "block" ? "tl-hug" : undefined} style={{ ...textCss(style, size), ...(bx?.mode === "block" ? { alignSelf: style.align === "left" ? "flex-start" : style.align === "right" ? "flex-end" : "center" } : null) }}>
        {lines.map((l, i) => (
          <p
            // a translation line (two languages line by line) takes the second-language look
            data-paired={l.startsWith(PAIRED) || undefined}
            // a new slide's lines are new elements, so their animation plays again
            key={style.enter && style.enter !== "none" ? `${key}\u0001${i}` : i}
            className={style.enter && style.enter !== "none" ? `tl-enter tl-enter--${style.enter}` : undefined}
            style={style.enter && style.enter !== "none" ? { animationDelay: `${i * (style.enter === "type" ? 700 : 140)}ms` } : undefined}
          >
            {lineBg ? <span className="tl-line" style={lineBg}>{renderLine(l.replace(PAIRED, ""))}</span> : renderLine(l.replace(PAIRED, ""))}
          </p>
        ))}
      </div>
    </div>
  );
}

/** ProPresenter's growing box. It belongs to the layer, not to one slide's words, so it
 *  stays put between slides and glides from the old words' size to the new ones'. It
 *  follows the newest words in the layer (the old ones may still be fading out). */
function GrowBox({ box }: { box: TextBox }) {
  const me = useRef<HTMLDivElement>(null);
  const [r, setR] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  useLayoutEffect(() => {
    const layer = me.current?.parentElement;
    if (!layer) return;
    let ro: ResizeObserver | null = null;
    let watched: Element | null = null;
    const measure = () => {
      const hugs = layer.querySelectorAll<HTMLElement>(".tl-hug");
      const t = hugs[hugs.length - 1];
      if (!t) return setR(null);
      if (t !== watched) {
        ro?.disconnect();
        ro = new ResizeObserver(() => requestAnimationFrame(measure));
        ro.observe(t);
        watched = t;
      }
      const frame = (layer.closest(".tv") as HTMLElement | null)?.clientHeight ?? 1080;
      const lb = layer.getBoundingClientRect();
      const tb = t.getBoundingClientRect();
      // the words' real extent (lines may be narrower than the block)
      let left = Infinity, right = -Infinity;
      for (const p of Array.from(t.children) as HTMLElement[]) {
        const range = document.createRange();
        range.selectNodeContents(p);
        for (const rr of Array.from(range.getClientRects())) {
          left = Math.min(left, rr.left);
          right = Math.max(right, rr.right);
        }
      }
      if (!isFinite(left)) (left = tb.left), (right = tb.right);
      // the frame may be shown scaled (Preview, thumbnails): work in the layer's own units
      const scale = layer.offsetWidth ? lb.width / layer.offsetWidth : 1;
      const px = (box.padX / 100) * frame;
      const py = (box.padY / 100) * frame;
      setR({ x: (left - lb.left) / scale - px, y: (tb.top - lb.top) / scale - py, w: (right - left) / scale + 2 * px, h: tb.height / scale + 2 * py });
    };
    measure();
    // new words arrive as a new element in the layer
    const mo = new MutationObserver(() => requestAnimationFrame(measure));
    mo.observe(layer, { childList: true, subtree: true, characterData: true });
    return () => (mo.disconnect(), ro?.disconnect());
  }, [box.padX, box.padY]);
  return (
    <div
      ref={me}
      className="tl-growbox"
      style={
        r
          ? {
              left: r.x,
              top: r.y,
              width: r.w,
              height: r.h,
              background: box.color,
              borderRadius: `${box.radius}cqh`,
              transition: box.animate ? "left 260ms ease, top 260ms ease, width 260ms ease, height 260ms ease" : "none",
            }
          : { display: "none" }
      }
    />
  );
}

function Ticker({ l }: { l: TickerLayer }) {
  return (
    <div className="ticker" style={{ background: l.fill }}>
      <div className="ticker__track" style={{ ...textCss(l.style, l.style.size), animationDuration: `${Math.max(4, 120 / Math.max(1, l.speed))}s` }}>
        <span>{l.text}</span>
        <span aria-hidden="true">{l.text}</span>
      </div>
    </div>
  );
}

function useClock(on: boolean) {
  const [t, setT] = useState(() => new Date());
  useEffect(() => {
    if (!on) return;
    const i = setInterval(() => setT(new Date()), 1000);
    return () => clearInterval(i);
  }, [on]);
  return t.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export interface EditorHooks {
  selected: string | null;
  onPointerDown: (e: React.PointerEvent, layer: Layer, handle: string | null) => void;
  guides?: { x: number[]; y: number[] };
}

export function ThemeView({
  theme: themeIn,
  data,
  phase = "static",
  showKey = 0,
  textKey = "",
  hideText = false,
  still = false,
  editor,
  children,
}: {
  theme: Theme;
  data: SlideData;
  phase?: Phase;
  showKey?: number;
  textKey?: string;
  /** Clear mode: keep background and shapes, hide text. */
  hideText?: boolean;
  /** Draw video backgrounds as their still picture (thumbnails, Preview). */
  still?: boolean;
  editor?: EditorHooks;
  children?: ReactNode;
}) {
  const xfMs = useMediaFadeMs();
  // brand references ("brand:2", font "brand") become the church's colours and fonts
  const brand = useBrand();
  const theme = useMemo(() => resolveBrand(themeIn, brand), [themeIn, brand]);
  const url = useMediaUrl();
  const clock = useClock(theme.layers.some((l) => l.kind === "text" && l.field === "clock"));
  const bg = theme.background;
  // two languages line by line: the words layer holds both, using the whole area
  const paired = !!theme.pairLines && data.second.length > 0;
  const secondLook = theme.layers.find((x): x is TextLayer => x.kind === "text" && x.field === "second")?.style;
  const secondEmpty = data.second.length === 0 || paired;
  // how the words change between slides: fade, slide up, wipe, zoom or push (cut: none)
  const fade = theme.transition.kind !== "cut" ? { animationDuration: `${theme.transition.ms}ms` } : undefined;
  const enter =
    { fade: "anim-xfade", "slide-up": "anim-slideup", wipe: "anim-wipe", zoom: "anim-zoom", push: "anim-push", cover: "anim-cover", cube: "anim-cube", drop: "anim-drop", iris: "anim-iris", pageflip: "anim-pageflip", cut: "" }[
      theme.transition.kind
    ] ?? "anim-xfade";

  const vid = bg.kind === "video" ? pickVideo(bg, data.rotateKey) : null;
  const tinted = bg.kind === "color" && /^#([0-9a-f]{8}|[0-9a-f]{4})$/i.test(bg.color.trim()) && !/f{2}$|^#[0-9a-f]{3}f$/i.test(bg.color.trim());
  // EasyWorship's background options: turn, flips and opacity
  const bgLook: CSSProperties = {
    objectFit: bg.fit,
    opacity: bg.opacity ?? 1,
    transform: bg.turn || bg.flipH || bg.flipV ? `rotate(${bg.turn ?? 0}deg) scale(${bg.flipH ? -1 : 1}, ${bg.flipV ? -1 : 1})` : undefined,
  };
  return (
    <div
      className={`tv ${bg.kind === "transparent" ? "tv--keyed" : ""} ${tinted ? "tv--tinted" : ""} ${editor ? "tv--editor" : ""}`}
      style={
        tinted
          ? ({ "--tv-tint": bg.color } as React.CSSProperties)
          : { background: bg.kind === "color" ? bg.color : bg.kind === "transparent" ? "transparent" : bg.kind === "gradient" && bg.gradient ? gradientCss(bg.gradient) : "#000" }
      }
    >
      {/* the background crossfades when it changes (a new song's video); the outgoing
          video keeps playing while it fades */}
      <Crossfade k={bg.kind === "image" ? `i${bg.image}` : bg.kind === "video" ? `v${vid?.video}` : "none"} ms={still || editor ? 0 : xfMs} className="tv__bgfade">
        {bg.kind === "image" && bg.image && <img className="tv__bg" src={url(bg.image)} alt="" style={bgLook} />}
        {bg.kind === "video" && vid?.video && (still ? (
          vid.poster ? <img className="tv__bg" src={url(vid.poster)} alt="" style={bgLook} /> : null
        ) : (
          // muted, looping, never restarted by slide changes (the element stays mounted;
          // a new item may pick another video from the set)
          <BgVideo key={vid.video} src={url(vid.video)} poster={vid.poster ? url(vid.poster) : undefined} style={bgLook} volume={data.sound ? bg.volume ?? 0 : 0} />
        ))}
      </Crossfade>
      {(bg.kind === "image" || bg.kind === "video") && bg.dim > 0 && <div className="tv__dim" style={{ opacity: bg.dim }} />}
      {theme.layers.map((l0) => {
        if (l0.hidden) return null;
        // layers for an item's first or last slide only (the editor shows them all)
        if (!editor && ((l0.onlyOn === "first" && data.first === false) || (l0.onlyOn === "last" && data.last === false))) return null;
        // a chorus, bridge… may look different from the verses
        const sec = theme.sections?.[sectionKindOf(data.section) ?? ("verse" as never)];
        const l1 =
          sec && l0.kind === "text" && (l0.field === "lyrics" || l0.field === "second")
            ? { ...l0, style: { ...l0.style, italic: sec.italic ?? l0.style.italic, weight: sec.weight ?? l0.style.weight, color: l0.field === "lyrics" && sec.color ? sec.color : l0.style.color } }
            : l0;
        // the operator's quick change (Live pane): the slide's own words only
        const sc = data.textScale ?? 1;
        const sh = data.textShift ?? 0;
        const l =
          !editor && (sc !== 1 || sh !== 0) && l1.kind === "text" && ["lyrics", "second", "reference", "section"].includes(l1.field)
            ? { ...l1, y: l1.y + sh, solo: l1.solo ? { ...l1.solo, y: l1.solo.y + sh } : l1.solo, style: { ...l1.style, size: l1.style.size * sc, minSize: l1.style.minSize * sc } }
            : l1;
        let rect = { x: l.x, y: l.y, w: l.w, h: l.h };
        let inner: ReactNode = null;
        if (l.kind === "text") {
          const lines = contentFor(l, data, clock, paired);
          if (l.solo && secondEmpty && l.field !== "second") rect = l.solo;
          const isSlideText = l.field === "lyrics" || l.field === "second" || l.field === "reference";
          // Clear: every text goes (words, verse labels, reference, title, copyright),
          // only the background, shapes and pictures stay; a clock is not slide text
          if (!editor && (lines.length === 0 || (hideText && l.field !== "clock"))) return null;
          inner = (
            <>
              {l.style.box?.mode === "block" && lines.length > 0 && <GrowBox box={l.style.box} />}
              <div key={isSlideText ? textKey : "fixed"} data-field={l.field} className={`tl-text ${isSlideText && fade && phase !== "static" ? enter : ""}`} style={isSlideText ? fade : undefined}>
                {lines.length ? (
                  <div
                    className="tl-pairhost"
                    style={
                      paired && l.field === "lyrics"
                        ? ({ "--wp-paired": secondLook?.color ?? "inherit", "--wp-paired-style": secondLook?.italic === false ? "normal" : "italic" } as CSSProperties)
                        : undefined
                    }
                  >
                    <FitText lines={lines} style={l.style} valign={l.style.valign} others={l.field === "lyrics" ? data.allPrimary : l.field === "second" ? data.allSecond : undefined} />
                  </div>
                ) : (
                  <div className="tl-placeholder">{l.name}</div>
                )}
              </div>
            </>
          );
        } else if (l.kind === "shape") {
          inner = <div className="tl-fill" style={{ background: l.gradient ? gradientCss(l.gradient) : l.fill, borderRadius: `${l.radius}cqh`, border: l.border.width ? `${l.border.width}cqh solid ${l.border.color}` : undefined }} />;
        } else if (l.kind === "image") {
          inner = l.src ? <img className="tl-fill" src={url(l.src)} alt="" style={{ objectFit: l.fit }} draggable={false} /> : editor ? <div className="tl-placeholder">Choose an image</div> : null;
        } else if (l.kind === "ticker") {
          inner = <Ticker l={l} />;
        }
        const a = animCss(l, phase);
        const sel = editor?.selected === l.id;
        return (
          <div
            key={`${l.id}-${showKey}`}
            className={`tl ${a.className} ${sel ? "is-selected" : ""} ${l.locked ? "is-locked" : ""}`}
            style={{ left: `${rect.x}%`, top: `${rect.y}%`, width: `${rect.w}%`, height: `${rect.h}%`, opacity: l.opacity, ...a.style }}
            onPointerDown={editor ? (e) => editor.onPointerDown(e, l, null) : undefined}
          >
            {inner}
            {sel && !l.locked && editor && (
              <>
                {["nw", "n", "ne", "e", "se", "s", "sw", "w"].map((h) => (
                  <span key={h} className={`handle handle--${h}`} onPointerDown={(e) => (e.stopPropagation(), editor.onPointerDown(e, l, h))} />
                ))}
              </>
            )}
          </div>
        );
      })}
      {editor?.guides?.x.map((g) => <div key={`gx${g}`} className="guide guide--v" style={{ left: `${g}%` }} />)}
      {editor?.guides?.y.map((g) => <div key={`gy${g}`} className="guide guide--h" style={{ top: `${g}%` }} />)}
      {children}
    </div>
  );
}
