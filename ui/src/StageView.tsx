import { useEffect, useState } from "react";
import type { Broadcast } from "./api";
import { fmtLeft, timerLeft, useNow } from "./Cues";

// Stage display for the pulpit and the band: what is on the screens now, what comes
// next, the time, and any note from the operator. Built to be read at a distance,
// so it ignores themes: plain white on black, big type, nothing decorative.

function useClock() {
  const [t, setT] = useState(() => new Date());
  useEffect(() => {
    const i = setInterval(() => setT(new Date()), 1000);
    return () => clearInterval(i);
  }, []);
  return t.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function StageView({ b, lost }: { b: Broadcast | null; lost: boolean }) {
  const clock = useClock();
  const s = b?.state;
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
              {cur.primary.map((l, i) => (
                <p key={i}>{l}</p>
              ))}
              {cur.second.length > 0 && (
                <div className="stage-view__second">
                  {cur.second.map((l, i) => (
                    <p key={i}>{l}</p>
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
              <p key={i}>{l}</p>
            ))}
          </>
        ) : (
          cur && !isMedia && <span className="stage-view__nextlabel">Last slide of this item</span>
        )}
      </footer>
    </div>
  );
}
