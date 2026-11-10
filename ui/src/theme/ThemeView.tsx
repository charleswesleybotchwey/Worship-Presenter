import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { useMediaUrl } from "./media";
import { fontStack, type Layer, type SlideData, type TextLayer, type TextStyle, type Theme, type TickerLayer } from "./types";

// Draws a theme with real content. Used by the theme editor, thumbnails, Preview,
// Live and the output windows, so what is designed is exactly what projects.
//
// phase "in" plays each layer's in-animation (when something goes on air),
// "out" plays the out-animation (going blank), "static" draws without motion.
// `textKey` changes on every slide change; text layers then cross-fade with the
// theme's transition while bands and images stay put.

export type Phase = "in" | "out" | "static";

function contentFor(l: TextLayer, d: SlideData, clock: string): string[] {
  switch (l.field) {
    case "lyrics":
      return d.primary;
    case "second":
      return d.second;
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
    textShadow: s.shadow.on ? `${s.shadow.x}cqh ${s.shadow.y}cqh ${s.shadow.blur}cqh ${s.shadow.color}` : "none",
    WebkitTextStroke: s.outline.on ? `${s.outline.width}cqh ${s.outline.color}` : undefined,
    paintOrder: "stroke fill",
  };
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
function FitText({ lines, style, valign }: { lines: string[]; style: TextStyle; valign: TextStyle["valign"] }) {
  const box = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState(style.size);
  const key = lines.join("\n");

  useLayoutEffect(() => {
    const b = box.current;
    const t = inner.current;
    if (!b || !t) return;
    const fit = () => {
      if (!style.autoFit) return setSize(style.size);
      // a smallest size that is not smaller than the size (an old built-in theme
      // had this) would stop text shrinking at all; allow down to 60 % then
      let lo = style.minSize < style.size ? style.minSize : style.size * 0.6;
      let hi = style.size;
      t.style.fontSize = `${hi}cqh`;
      if (t.scrollHeight <= b.clientHeight + 1 && t.scrollWidth <= b.clientWidth + 1) return setSize(hi);
      for (let i = 0; i < 8; i++) {
        const mid = (lo + hi) / 2;
        t.style.fontSize = `${mid}cqh`;
        if (t.scrollHeight <= b.clientHeight + 1 && t.scrollWidth <= b.clientWidth + 1) lo = mid;
        else hi = mid;
      }
      setSize(lo);
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

  return (
    <div ref={box} className={`tl-box tl-box--${valign}`}>
      <div ref={inner} style={textCss(style, size)}>
        {lines.map((l, i) => (
          <p key={i}>{l}</p>
        ))}
      </div>
    </div>
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
  theme,
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
  const url = useMediaUrl();
  const clock = useClock(theme.layers.some((l) => l.kind === "text" && l.field === "clock"));
  const bg = theme.background;
  const secondEmpty = data.second.length === 0;
  const fade = theme.transition.kind === "fade" ? { animationDuration: `${theme.transition.ms}ms` } : undefined;

  return (
    <div className={`tv ${bg.kind === "transparent" ? "tv--keyed" : ""} ${editor ? "tv--editor" : ""}`} style={{ background: bg.kind === "color" ? bg.color : bg.kind === "transparent" ? "transparent" : "#000" }}>
      {bg.kind === "image" && bg.image && <img className="tv__bg" src={url(bg.image)} alt="" style={{ objectFit: bg.fit }} />}
      {bg.kind === "video" && bg.video && (still || !bg.video ? (
        bg.poster ? <img className="tv__bg" src={url(bg.poster)} alt="" style={{ objectFit: bg.fit }} /> : null
      ) : (
        // muted, looping, never restarted by slide changes (the element stays mounted)
        <video className="tv__bg" src={url(bg.video)} poster={bg.poster ? url(bg.poster) : undefined} style={{ objectFit: bg.fit }} autoPlay muted loop playsInline />
      ))}
      {(bg.kind === "image" || bg.kind === "video") && bg.dim > 0 && <div className="tv__dim" style={{ opacity: bg.dim }} />}
      {theme.layers.map((l) => {
        if (l.hidden) return null;
        let rect = { x: l.x, y: l.y, w: l.w, h: l.h };
        let inner: ReactNode = null;
        if (l.kind === "text") {
          const lines = contentFor(l, data, clock);
          if (l.solo && secondEmpty && l.field !== "second") rect = l.solo;
          const isSlideText = l.field === "lyrics" || l.field === "second" || l.field === "reference";
          if (!editor && (lines.length === 0 || (hideText && isSlideText))) return null;
          inner = (
            <div key={isSlideText ? textKey : "fixed"} className={`tl-text ${isSlideText && fade && phase !== "static" ? "anim-xfade" : ""}`} style={isSlideText ? fade : undefined}>
              {lines.length ? <FitText lines={lines} style={l.style} valign={l.style.valign} /> : <div className="tl-placeholder">{l.name}</div>}
            </div>
          );
        } else if (l.kind === "shape") {
          inner = <div className="tl-fill" style={{ background: l.fill, borderRadius: `${l.radius}cqh`, border: l.border.width ? `${l.border.width}cqh solid ${l.border.color}` : undefined }} />;
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
