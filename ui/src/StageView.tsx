import { stripRich } from "./rich";
import { pieces, prefersFlats, transpose } from "./chords";
import { useEffect, useState } from "react";
import { api, onSettingsChanged, type Broadcast } from "./api";
import { fmtLeft, timerLeft, useNow } from "./Cues";

// Stage display for the pulpit and the band: what is on the screens now, what comes
// next, the time, and any note from the operator. Built to be read at a distance,
// so it ignores themes: plain white on black, big type, nothing decorative.

/** A box on a custom stage layout: which part, where (% of the screen), how it looks. */
export type StagePart = "now" | "next" | "clock" | "timer" | "title" | "messages" | "key";
export interface StageBox {
  part: StagePart;
  x: number;
  y: number;
  w: number;
  h: number;
  /** text size, % of the screen height */
  size: number;
  color: string;
  bg: string;
  align: "left" | "center" | "right";
}
export interface StageLayout {
  boxes: StageBox[];
}
export const STAGE_PARTS: { part: StagePart; label: string }[] = [
  { part: "now", label: "Words now" },
  { part: "next", label: "Next slide" },
  { part: "clock", label: "Clock" },
  { part: "timer", label: "Timer" },
  { part: "title", label: "Item, section and count" },
  { part: "messages", label: "Messages and alerts" },
  { part: "key", label: "Song key" },
];
const box = (part: StagePart, x: number, y: number, w: number, h: number, size: number, extra: Partial<StageBox> = {}): StageBox => ({ part, x, y, w, h, size, color: "#ffffff", bg: "transparent", align: "left", ...extra });
export const STAGE_PRESETS: Record<string, { label: string; layout: StageLayout }> = {
  words: {
    label: "Big words, next slide, clock",
    layout: { boxes: [box("title", 2, 2, 70, 7, 4, { color: "#aaaaaa" }), box("clock", 74, 2, 24, 7, 5, { align: "right" }), box("now", 2, 11, 96, 60, 7), box("messages", 2, 72, 96, 6, 3.5, { color: "#ffcc00" }), box("next", 2, 79, 96, 19, 4, { color: "#9ad0ff" })] },
  },
  pastor: {
    label: "Pastor: big clock and timer",
    layout: { boxes: [box("clock", 2, 2, 46, 26, 18), box("timer", 52, 2, 46, 26, 18, { align: "right", color: "#ffcc00" }), box("messages", 2, 30, 96, 10, 5, { color: "#ffcc00" }), box("now", 2, 42, 96, 40, 5), box("next", 2, 84, 96, 14, 3.5, { color: "#9ad0ff" })] },
  },
  band: {
    label: "Band: key, words with chords, next",
    layout: { boxes: [box("key", 2, 2, 30, 9, 6, { color: "#7CFC00" }), box("title", 34, 2, 44, 9, 4, { color: "#aaaaaa" }), box("clock", 80, 2, 18, 9, 5, { align: "right" }), box("now", 2, 13, 96, 58, 6), box("next", 2, 74, 96, 24, 4.5, { color: "#9ad0ff" })] },
  },
};

/** The custom layout from Settings, kept up to date (null: the standard layout). */
function useStageLayout(): StageLayout | null {
  const [l, setL] = useState<StageLayout | null>(null);
  useEffect(() => {
    const load = () =>
      api
        .getSettings()
        .then((x) => {
          try {
            const v = x.stage_layout ? (JSON.parse(x.stage_layout) as StageLayout) : null;
            setL(v?.boxes?.length ? v : null);
          } catch {
            setL(null);
          }
        })
        .catch(() => {});
    load();
    let off = () => {};
    onSettingsChanged(load).then((u) => (off = u));
    return () => off();
  }, []);
  return l;
}

function useClock() {
  const [t, setT] = useState(() => new Date());
  useEffect(() => {
    const i = setInterval(() => setT(new Date()), 1000);
    return () => clearInterval(i);
  }, []);
  return t.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function StageView({ b, lost, layout: forced }: { b: Broadcast | null; lost: boolean; layout?: StageLayout | null }) {
  const clock = useClock();
  const saved = useStageLayout();
  const layout = forced !== undefined ? forced : saved;
  const s = b?.state;
  const shift = s?.transpose ?? 0;
  const flats = prefersFlats(b?.song_key, shift);
  const slides = b?.slides ?? [];
  const cur = s ? slides[s.slide_idx] : undefined;
  const next = s ? slides[s.slide_idx + 1] : undefined;
  const title = b?.item ? (b.item.kind === "scripture" ? b.caption : b.item.title) : null;
  const isDeck = b?.item?.kind === "announcement";
  const screens = !b || lost ? "No signal from the control computer" : s?.mode === "blank" ? "Screens blank" : s?.mode === "logo" ? "Logo on screens" : s?.mode === "clear" ? "Words hidden" : null;
  const isMedia = b?.item?.kind === "media";
  const timer = s?.timer?.on_stage ? s.timer : null;
  const now = useNow(!!timer || !!s?.auto);
  const left = timer ? timerLeft(timer, now) : 0;
  const alert = s?.alert?.on_stage && !(s.alert.until_ms && now >= s.alert.until_ms) ? s.alert : null;

  // the parts, shared by the standard and the custom layouts
  const keyText = b?.song_key ? `Key ${shift ? `${transpose(b.song_key, shift, flats)} (from ${b.song_key}, ${shift > 0 ? "+" : ""}${shift})` : b.song_key}` : null;
  const nowPart = isMedia ? (
    <p className="stage-view__media">{b?.media?.kind === "video" ? `Video playing: ${b.media.title}` : `Picture: ${b?.media?.title ?? ""}`}</p>
  ) : (
    cur && (
      <>
        {isDeck && <p className="stage-view__decktitle">{cur.image ? "Picture" : cur.label}</p>}
        {cur.chords?.length ? cur.chords.map((line, i) => <ChordLine key={i} line={line || cur.primary[i] || ""} n={shift} flats={flats} />) : cur.primary.map((l, i) => <p key={i}>{stripRich(l)}</p>)}
        {cur.second.length > 0 && (
          <div className="stage-view__second">
            {cur.second.map((l, i) => (
              <p key={i}>{stripRich(l)}</p>
            ))}
          </div>
        )}
      </>
    )
  );
  const nextPart = next ? (
    <>
      <span className="stage-view__nextlabel">Next{next.label !== cur?.label ? ` · ${next.label}` : ""}</span>
      {next.primary.map((l, i) => (
        <p key={i}>{stripRich(l)}</p>
      ))}
    </>
  ) : (
    cur && !isMedia && <span className="stage-view__nextlabel">Last slide of this item</span>
  );
  const timerPart = timer && (
    <span className={left < 0 ? "is-over" : ""}>
      {fmtLeft(left)} <small>{timer.paused_left_ms != null ? `${timer.label || "Timer"}, paused` : timer.label}</small>
    </span>
  );
  const messagesPart = (
    <>
      {s?.repeat && (
        <div>
          {s.repeat.label} <strong>{Math.min(s.repeat.done, 99)}</strong> of {s.repeat.total}
          {s.repeat.done >= s.repeat.total && <span> · last time</span>}
        </div>
      )}
      {s?.lining_out && <div>Lining out</div>}
      {s?.stage_message && <div>{s.stage_message}</div>}
      {alert && <div>Alert on screens: {alert.text}</div>}
      {screens && <div>{screens}</div>}
    </>
  );
  const titlePart = (
    <>
      {title ?? "Nothing live"}
      {cur && !isMedia && <span className="stg-box__label"> · {cur.label}</span>}
      {slides.length > 1 && s && (
        <span className="stg-box__label">
          {" "}
          · {s.slide_idx + 1}/{slides.length}
        </span>
      )}
    </>
  );
  if (layout?.boxes.length) {
    const parts: Record<StagePart, React.ReactNode> = { now: nowPart, next: nextPart, clock, timer: timerPart, title: titlePart, messages: messagesPart, key: keyText };
    return (
      <div className="stage-view stage-view--custom">
        {layout.boxes.map((bx, i) => (
          <div
            key={i}
            className={`stg-box stg-box--${bx.part}`}
            style={{ left: `${bx.x}%`, top: `${bx.y}%`, width: `${bx.w}%`, height: `${bx.h}%`, fontSize: `${bx.size}cqh`, color: bx.color, background: bx.bg, textAlign: bx.align }}
          >
            {parts[bx.part]}
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="stage-view">
      <header className="stage-view__top">
        <span className="stage-view__title">{title ?? "Nothing live"}</span>
        {cur && !isMedia && <span className="stage-view__label">{cur.label}</span>}
        {slides.length > 1 && s && (
          <span className="stage-view__count">
            {s.slide_idx + 1}/{slides.length}
          </span>
        )}
        {b?.song_key && (
          <span className="stage-view__key" title="Key for the musicians">
            Key {shift ? `${transpose(b.song_key, shift, flats)} (from ${b.song_key}, ${shift > 0 ? "+" : ""}${shift})` : b.song_key}
          </span>
        )}
        <span className="stage-view__clock">{clock}</span>
      </header>
      {timer && (
        <div className={`stage-view__timer ${left < 0 ? "is-over" : ""}`}>
          {fmtLeft(left)} <small>{timer.paused_left_ms != null ? `${timer.label || "Timer"}, paused` : timer.label}</small>
        </div>
      )}
      {s?.repeat && (
        <div className="stage-view__repeat">
          {s.repeat.label} <strong>{Math.min(s.repeat.done, 99)}</strong> of {s.repeat.total}
          {s.repeat.done >= s.repeat.total && <span> · last time</span>}
        </div>
      )}
      {s?.lining_out && <div className="stage-view__lining">Lining out</div>}
      {s?.auto && !s.auto.paused && b?.item?.kind === "announcement" && (
        <div className="stage-view__lining wp-num">Next slide in {Math.max(0, Math.ceil(((s.auto.durations_ms[s.slide_idx] ?? 0) - (now - s.slide_since_ms)) / 1000))} s</div>
      )}
      {s?.stage_message && <div className="stage-view__message">{s.stage_message}</div>}
      {alert && <div className="stage-view__alert">Alert on screens: {alert.text}</div>}
      {screens && <div className="stage-view__state">{screens}</div>}
      <main className="stage-view__now">
        {isMedia ? (
          <p className="stage-view__media">{b?.media?.kind === "video" ? `Video playing: ${b.media.title}` : `Picture: ${b?.media?.title ?? ""}`}</p>
        ) : (
          cur && (
            <>
              {isDeck && <p className="stage-view__decktitle">{cur.image ? "Picture" : cur.label}</p>}
              {cur.chords?.length
                ? cur.chords.map((line, i) => <ChordLine key={i} line={line || cur.primary[i] || ""} n={shift} flats={flats} />)
                : cur.primary.map((l, i) => <p key={i}>{stripRich(l)}</p>)}
              {cur.second.length > 0 && (
                <div className="stage-view__second">
                  {cur.second.map((l, i) => (
                    <p key={i}>{stripRich(l)}</p>
                  ))}
                </div>
              )}
            </>
          )
        )}
      </main>
      <footer className={`stage-view__next ${s?.lining_out ? "stage-view__next--big" : ""}`}>
        {next ? (
          <>
            <span className="stage-view__nextlabel">Next{next.label !== cur?.label ? ` · ${next.label}` : ""}</span>
            {next.primary.map((l, i) => (
              <p key={i}>{stripRich(l)}</p>
            ))}
          </>
        ) : (
          cur && !isMedia && <span className="stage-view__nextlabel">Last slide of this item</span>
        )}
      </footer>
    </div>
  );
}

/** A lyric line with its chords above the words, moved by the transpose setting. */
function ChordLine({ line, n, flats }: { line: string; n: number; flats: boolean }) {
  const ps = pieces(line);
  if (!ps.some((p) => p.chord)) return <p>{line}</p>;
  return (
    <p className="chordline">
      {ps.map((p, i) => (
        <span key={i} className="chordline__piece">
          <span className="chordline__chord">{p.chord ? transpose(p.chord, n, flats) : "\u00a0"}</span>
          <span className="chordline__text">{p.text || "\u00a0"}</span>
        </span>
      ))}
    </p>
  );
}
