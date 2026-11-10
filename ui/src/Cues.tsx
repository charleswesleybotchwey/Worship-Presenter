import { useEffect, useState } from "react";
import type { Alert, Timer } from "./api";

// Timer and alert drawing shared by the outputs and the stage display.

export function useNow(on: boolean, every = 250) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    const i = setInterval(() => setNow(Date.now()), every);
    return () => clearInterval(i);
  }, [on, every]);
  return now;
}

/** "4:59", "1:02:03", or "+0:12" once over. */
export function fmtLeft(ms: number) {
  const over = ms < 0;
  const s = Math.ceil(Math.abs(ms) / 1000) - (over ? 1 : 0);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const body = h ? `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}` : `${m}:${String(sec).padStart(2, "0")}`;
  return over ? `+${body}` : body;
}

export const timerLeft = (t: Timer, now: number) => t.paused_left_ms ?? t.ends_at_ms - now;

export function TimerOverlay({ t, full }: { t: Timer; full: boolean }) {
  const now = useNow(true);
  const left = timerLeft(t, now);
  const over = left < 0;
  return (
    <div className={`cue-timer ${full ? "cue-timer--full" : ""} ${over ? "is-over" : ""}`} role="timer">
      {t.label && <span className="cue-timer__label">{t.label}</span>}
      <span className="cue-timer__time">{fmtLeft(left)}</span>
    </div>
  );
}

export function AlertOverlay({ a }: { a: Alert }) {
  const now = useNow(!!a.until_ms, 500);
  if (a.until_ms && now >= a.until_ms) return null;
  return (
    <div className={`cue-alert cue-alert--${a.position} cue-alert--${a.style}`} role="alert">
      {a.style === "ticker" ? (
        <div className="cue-alert__track" style={{ animationDuration: `${Math.max(10, a.text.length * 0.28)}s` }}>
          <span>{a.text}</span>
          <span aria-hidden="true">{a.text}</span>
        </div>
      ) : (
        <span>{a.text}</span>
      )}
    </div>
  );
}
