// The service window's live controls and small parts (moved out of Control.tsx).

import { useEffect, useRef, useState } from "react";
import { api, type LiveState, type MediaItem, type Mode, type Slide, type Timer } from "../../api";
import { fmtLeft, timerLeft, useNow } from "../../Cues";
import { Icon } from "../../Icon";
import { OutputDots } from "../../OutputsPanel";
import { useMediaUrl } from "../../theme/media";

export function ServiceActions({ state, onMode }: { state: LiveState | undefined; latched?: boolean; onMode: (m: Mode) => void; onResume?: () => void }) {
  // a switch: on, it stands out in its own colour; pressed again, the words come back
  const tips: Record<string, string> = { blank: "Black screens", logo: "The logo on the screens", clear: "Hide the words, keep the background" };
  const btn = (m: Mode, label: string, icon: string) => {
    const on = state?.mode === m;
    return (
      <button className={`wp-btn wp-mode ${on ? `is-on is-on--${m}` : ""}`} aria-pressed={on} onClick={() => onMode(state?.mode === m ? "live" : m)} title={on ? `${label} is on: press again to bring the words back` : tips[m]}>
        {on && <span className="wp-mode__dot" aria-hidden="true" />}
        <Icon name={icon} /> {label}
        {on && <span className="wp-mode__on">ON</span>}
      </button>
    );
  };
  return (
    <div className="wp-service-actions" role="group" aria-label="Screens">
      {btn("blank", "Blank", "blank")}
      {btn("logo", "Logo", "logo")}
      {btn("clear", "Clear", "clear")}
      <ScreensSwitch />
      <OutputDots onOpen={() => window.dispatchEvent(new CustomEvent("wp-outputs"))} />
    </div>
  );
}

export function TimerChip({ timer, onClick }: { timer: Timer | null; onClick: () => void }) {
  const now = useNow(!!timer, 500);
  const left = timer ? timerLeft(timer, now) : 0;
  return (
    <button className={`wp-btn wp-btn--quiet wp-num ${timer ? "is-on" : ""} ${timer && left < 0 ? "is-over" : ""}`} onClick={onClick}>
      <Icon name="timer" /> {timer ? fmtLeft(left) + (timer.paused_left_ms != null ? " paused" : "") : "Timer"}
    </button>
  );
}

export function AutoBar({ state, onPause }: { state: LiveState; onPause: (paused: boolean) => void }) {
  const now = useNow(!state.auto?.paused, 500);
  const a = state.auto!;
  const d = a.durations_ms[state.slide_idx] ?? 0;
  const left = Math.max(0, Math.ceil((d - (now - state.slide_since_ms)) / 1000));
  const last = state.slide_idx >= a.durations_ms.length - 1 && !a.looping;
  return (
    <div className="wp-livebar">
      <span className="wp-num">{a.paused ? "Paused" : last ? "Last slide" : `Next slide in ${left} s`}</span>
      {a.looping && <span className="wp-note wp-note--inline">Loops</span>}
      <span className="wp-spacer" />
      <button className="wp-btn" onClick={() => onPause(!a.paused)}>
        {a.paused ? "Resume advancing" : "Pause advancing"}
      </button>
    </div>
  );
}

export function RepeatBar({ state, slides, onSet }: { state: LiveState | undefined; slides: Slide[]; onSet: (label: string | null, total: number, done: number | null) => void }) {
  const labels = [...new Set(slides.map((s) => s.label))];
  const [label, setLabel] = useState(labels.find((l) => l.startsWith("C")) ?? labels[0] ?? "C");
  const [total, setTotal] = useState(3);
  const r = state?.repeat;
  if (!labels.length) return null;
  return (
    <div className="wp-livebar">
      {r ? (
        <>
          <span className="wp-num wp-livebar__count">
            {r.label}: {Math.min(r.done, 99)} of {r.total}
          </span>
          <button className="wp-icon-btn" aria-label="One fewer" onClick={() => onSet(r.label, r.total, Math.max(0, r.done - 1))}>
            −
          </button>
          <button className="wp-icon-btn" aria-label="One more" onClick={() => onSet(r.label, r.total, r.done + 1)}>
            +
          </button>
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--quiet" onClick={() => onSet(null, 1, null)}>
            Stop counting
          </button>
        </>
      ) : (
        <>
          <span>Count</span>
          <select className="wp-input" aria-label="Section to count" value={label} onChange={(e) => setLabel(e.target.value)}>
            {labels.map((l) => (
              <option key={l}>{l}</option>
            ))}
          </select>
          <span>×</span>
          <select className="wp-input" aria-label="Times" value={total} onChange={(e) => setTotal(Number(e.target.value))}>
            {[2, 3, 4, 5, 6, 8].map((n) => (
              <option key={n}>{n}</option>
            ))}
          </select>
          <button className="wp-btn wp-btn--quiet" onClick={() => onSet(label, total, null)} title="The stage display shows the count large for the band">
            Start counting
          </button>
        </>
      )}
    </div>
  );
}

export function Clock() {
  const now = useNow(true, 1000);
  return <span className="wp-num">{new Date(now).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>;
}

export function ViewToggle({ value, onChange }: { value: "list" | "thumbs"; onChange: (v: "list" | "thumbs") => void }) {
  return (
    <div className="wp-tabs ew-view" role="radiogroup" aria-label="Show slides as">
      <button role="radio" aria-checked={value === "list"} aria-selected={value === "list"} onClick={() => onChange("list")} title="Text list">
        List
      </button>
      <button role="radio" aria-checked={value === "thumbs"} aria-selected={value === "thumbs"} onClick={() => onChange("thumbs")} title="Pictures">
        Pictures
      </button>
    </div>
  );
}

/** A video or sound file in Preview, with EasyWorship-style controls: play, skip, seek,
 *  loop (also how it plays when live) and volume; muted to begin with. */
export function PreviewMedia({ item }: { item: MediaItem }) {
  const url = useMediaUrl();
  const src = url(`media:${item.path}`);
  const el = useRef<HTMLVideoElement & HTMLAudioElement>(null);
  const [t, setT] = useState(0);
  const [dur, setDur] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [muted, setMuted] = useState(true);
  const [vol, setVol] = useState(0.8);
  const [loop, setLoop] = useState(item.repeat === "loop");
  useEffect(() => setLoop(item.repeat === "loop"), [item.id, item.repeat]);
  const m = () => el.current;
  const fmt = (x: number) => (isFinite(x) ? `${Math.floor(x / 60)}:${String(Math.floor(x % 60)).padStart(2, "0")}` : "0:00");
  const common = {
    ref: el,
    src,
    preload: "metadata" as const,
    loop,
    muted,
    onTimeUpdate: () => setT(m()?.currentTime ?? 0),
    onLoadedMetadata: () => (setDur(m()?.duration ?? 0), m() && (m()!.volume = vol)),
    onPlay: () => setPlaying(true),
    onPause: () => setPlaying(false),
    onEnded: () => setPlaying(false),
  };
  return (
    <div className="pmx">
      {item.kind === "video" ? (
        <video className="ew-preview-media" playsInline {...common} onClick={() => (m()?.paused ? m()?.play() : m()?.pause())} />
      ) : (
        <div className="ew-preview-media ew-preview-media--audio">
          <span>♪ {item.title}</span>
          <audio {...common} />
        </div>
      )}
      <div className="pmx__bar">
        <input
          className="pmx__seek"
          type="range"
          min={0}
          max={dur || 0}
          step={0.1}
          value={t}
          onChange={(e) => m() && (m()!.currentTime = Number(e.target.value))}
          aria-label="Position"
        />
        <span className="pmx__time wp-num">
          {fmt(t)} / {fmt(dur)}
        </span>
      </div>
      <div className="pmx__bar">
        <button className="pmx__play" onClick={() => (m()?.paused ? void m()?.play() : m()?.pause())} aria-label={playing ? "Pause" : "Play"} title={playing ? "Pause" : "Play"}>
          {playing ? "❚❚" : "▶"}
        </button>
        <button className="wp-icon-btn" onClick={() => m() && (m()!.currentTime = Math.max(0, m()!.currentTime - 10))} aria-label="Back 10 seconds" title="Back 10 seconds">
          «
        </button>
        <button className="wp-icon-btn" onClick={() => m() && (m()!.currentTime = Math.min(dur, m()!.currentTime + 10))} aria-label="Forward 10 seconds" title="Forward 10 seconds">
          »
        </button>
        <span className="wp-spacer" />
        <button
          className={`wp-icon-btn ${loop ? "is-on" : ""}`}
          aria-pressed={loop}
          title={loop ? "Loops (also when live): click to play once" : "Plays once: click to loop (also when live)"}
          onClick={() => {
            const next = !loop;
            setLoop(next);
            void api.setMediaOptions(item.id, next ? "loop" : "once", null).catch(() => {});
          }}
        >
          ⟲
        </button>
        <button className="wp-icon-btn" onClick={() => setMuted((x) => !x)} aria-label={muted ? "Unmute" : "Mute"} title={muted ? "Muted here (the screens are not affected): click to listen" : "Mute"}>
          {muted ? "🔇" : "🔊"}
        </button>
        <input
          className="pmx__vol"
          type="range"
          min={0}
          max={1}
          step={0.05}
          value={vol}
          onChange={(e) => {
            const v = Number(e.target.value);
            setVol(v);
            if (m()) m()!.volume = v;
            if (v > 0) setMuted(false);
          }}
          aria-label="Volume"
        />
      </div>
    </div>
  );
}

/** EasyWorship's Live button: every screen off (and back on, the same ones). */
export function ScreensSwitch() {
  const [on, setOn] = useState(true);
  useEffect(() => {
    api.screensAreOn().then(setOn).catch(() => {});
  }, []);
  return (
    <button
      className={`wp-btn wp-mode ${on ? "" : "is-on is-on--blank"}`}
      aria-pressed={!on}
      title={on ? "Switch every screen off (projector, stage, NDI); press again to bring them back" : "Screens are off: press to switch them back on"}
      onClick={() => void api.screensPower(!on).then(setOn)}
    >
      {!on && <span className="wp-mode__dot" aria-hidden="true" />}
      {on ? "Screens" : "Screens off"}
    </button>
  );
}
