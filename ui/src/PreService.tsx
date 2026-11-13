import { useEffect, useState } from "react";
import { api, type DeckSummary, type ProgramItem, type TimerShow } from "./api";

// One button before church: a countdown to the start time, the announcements turning by
// themselves, background music, and on time the first item of the service goes live
// (the music fades as it always does on Go live), or the logo shows.

export interface PreServicePlan {
  startsAt: string; // "HH:MM"
  deckId: number | null;
  seconds: number;
  countdown: TimerShow;
  music: boolean;
  onTime: "first" | "logo" | "stop";
}

const nextHalfHour = () => {
  const d = new Date(Date.now() + 15 * 60_000);
  d.setMinutes(d.getMinutes() < 30 ? 30 : 60, 0, 0);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
};

export function loadPlan(): PreServicePlan {
  try {
    const p = JSON.parse(localStorage.getItem("wp-preservice") || "{}");
    return { startsAt: nextHalfHour(), deckId: null, seconds: 8, countdown: "corner", music: true, onTime: "first", ...p, ...(p.startsAt ? {} : { startsAt: nextHalfHour() }) };
  } catch {
    return { startsAt: nextHalfHour(), deckId: null, seconds: 8, countdown: "corner", music: true, onTime: "first" };
  }
}

/** Milliseconds until today's HH:MM (tomorrow's if it has passed by more than an hour). */
export function msUntil(hhmm: string): number {
  const [h, m] = hhmm.split(":").map(Number);
  const t = new Date();
  t.setHours(h, m, 0, 0);
  let ms = t.getTime() - Date.now();
  if (ms < -3_600_000) ms += 86_400_000;
  return ms;
}

export function PreServicePanel({
  firstItem,
  running,
  onStart,
  onStop,
  onClose,
}: {
  firstItem: ProgramItem | null;
  running: PreServicePlan | null;
  onStart: (p: PreServicePlan) => void;
  onStop: () => void;
  onClose: () => void;
}) {
  const [plan, setPlan] = useState<PreServicePlan>(loadPlan);
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    api
      .listAnnouncements()
      .then((d) => {
        setDecks(d);
        setPlan((p) => (p.deckId && d.some((x) => x.id === p.deckId) ? p : { ...p, deckId: d[0]?.id ?? null }));
      })
      .catch(() => {});
  }, []);
  const set = (p: Partial<PreServicePlan>) => setPlan((x) => ({ ...x, ...p }));
  const start = async () => {
    if (msUntil(plan.startsAt) <= 0) return setProblem("That time has passed. Choose the time the service starts.");
    if (plan.onTime === "first" && !firstItem) return setProblem("The schedule is empty: add the first item (call to worship), or choose “Show the logo”.");
    localStorage.setItem("wp-preservice", JSON.stringify(plan));
    onStart(plan);
  };
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog ps" role="dialog" aria-modal="true" aria-label="Pre-service">
        <header className="wp-dialog__head">
          <h2>Before the service</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        {running ? (
          <div className="ps__body">
            <p>
              Running: countdown to <strong>{running.startsAt}</strong>, announcements turning{running.music ? ", music playing" : ""}. On time:{" "}
              {running.onTime === "first" ? "the first item goes live" : running.onTime === "logo" ? "the logo shows" : "the countdown stops"}.
            </p>
            <button className="wp-btn" onClick={onStop}>
              Stop the pre-service loop
            </button>
          </div>
        ) : (
          <div className="ps__body">
            <label className="ps__row">
              <span>Service starts at</span>
              <input type="time" className="wp-input" value={plan.startsAt} onChange={(e) => set({ startsAt: e.target.value })} aria-label="Service starts at" />
            </label>
            <label className="ps__row">
              <span>Announcements</span>
              <select className="wp-input" value={plan.deckId ?? ""} onChange={(e) => set({ deckId: e.target.value ? Number(e.target.value) : null })} aria-label="Announcements">
                <option value="">None (countdown only)</option>
                {decks.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.title} ({d.slides} slides)
                  </option>
                ))}
              </select>
            </label>
            {plan.deckId && (
              <label className="ps__row">
                <span>Each slide for</span>
                <input type="number" className="wp-input" min={3} max={60} value={plan.seconds} onChange={(e) => set({ seconds: Number(e.target.value) || 8 })} aria-label="Seconds per slide" /> seconds
              </label>
            )}
            <label className="ps__row">
              <span>Countdown</span>
              <select className="wp-input" value={plan.countdown} onChange={(e) => set({ countdown: e.target.value as TimerShow })} aria-label="Countdown">
                <option value="corner">In the corner of the screens</option>
                <option value="full">Full screen</option>
                <option value="off">Stage display only</option>
              </select>
            </label>
            <label className="wp-check">
              <input type="checkbox" checked={plan.music} onChange={(e) => set({ music: e.target.checked })} /> Play background music
            </label>
            <label className="ps__row">
              <span>On time</span>
              <select className="wp-input" value={plan.onTime} onChange={(e) => set({ onTime: e.target.value as PreServicePlan["onTime"] })} aria-label="On time">
                <option value="first">Go live with the first schedule item{firstItem && "title" in firstItem ? ` (${firstItem.title})` : firstItem?.kind === "scripture" ? ` (${firstItem.reference})` : ""}</option>
                <option value="logo">Show the logo</option>
                <option value="stop">Stop the countdown only</option>
              </select>
            </label>
            {problem && <p className="form-error">{problem}</p>}
            <div className="ps__foot">
              <button className="wp-btn wp-btn--primary" onClick={start}>
                Start
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
