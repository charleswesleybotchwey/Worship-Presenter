import { useEffect, useRef, useState } from "react";
import { api, type SpeechStatus } from "./api";

// The Live Transcript panel: start/stop, the microphone level, the running transcript
// with confidence, the scriptures detected, and past sessions. A side drawer, so the
// service window stays usable behind it.

const conf = (c: number | null | undefined) => (c == null ? "none" : c >= 0.8 ? "high" : c >= 0.6 ? "mid" : "low");
const confTip = (c: number | null | undefined) => (c == null ? "No confidence from this engine" : `${Math.round(c * 100)}% confident`);

export function TranscriptPanel({
  speech,
  onStart,
  onStop,
  onPreview,
  onLive,
  onClose,
}: {
  speech: SpeechStatus | null;
  onStart: () => void;
  onStop: () => void;
  onPreview: (reference: string) => void;
  onLive: (reference: string) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<"live" | "history">("live");
  // a quicker look at the level while the panel is open
  const [fast, setFast] = useState<SpeechStatus | null>(null);
  useEffect(() => {
    if (!speech?.listening) {
      setFast(null);
      return;
    }
    const t = setInterval(() => api.speechStatus().then(setFast).catch(() => {}), 200);
    return () => clearInterval(t);
  }, [speech?.listening]);
  const s = fast ?? speech;
  const listening = !!s?.listening;
  const level = Math.min(1, s?.level ?? 0);
  const [mics, setMics] = useState<string[]>([]);
  const [mic, setMic] = useState("");
  const [gain, setGain] = useState<number | null>(null);
  useEffect(() => {
    api.speechMics().then(setMics).catch(() => {});
    api.getSettings().then((x) => setMic(x.speech_mic ?? "")).catch(() => {});
  }, []);
  useEffect(() => {
    if (gain == null && s?.gain != null) setGain(s.gain);
  }, [s?.gain, gain]);
  const bottom = useRef<HTMLDivElement>(null);
  const lines = s?.lines ?? [];
  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [lines.length, s?.caption]);
  const lastText = lines[lines.length - 1]?.[1] ?? "";
  const interim = listening && s?.caption && s.caption !== lastText && !s.caption.endsWith("…") ? s.caption : "";

  return (
    <aside className="trp" aria-label="Live Transcript">
      <header className="trp__head">
        <strong className="trp__title">Live transcript</strong>
        <span className="wp-note wp-note--inline">{s?.listening ?? ""}</span>
        <span className="wp-spacer" />
        <button className="wp-icon-btn" aria-label="Close" onClick={onClose}>
          ✕
        </button>
      </header>
      <div className="trp__controls">
        {s?.starting ? (
          <button className="wp-btn" disabled>
            <span className="trp__spin" aria-hidden="true" /> Starting…
          </button>
        ) : listening ? (
          <button className="wp-btn trp__stop" onClick={onStop}>
            Stop transcribing
          </button>
        ) : (
          <button className="wp-btn wp-btn--primary" onClick={onStart}>
            Start transcribing
          </button>
        )}
        <div className="trp__meter" role="meter" aria-label="Microphone level" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(level * 100)}>
          <div className={`trp__meter-fill ${level > 0.85 ? "is-red" : level > 0.6 ? "is-yellow" : ""}`} style={{ width: `${level * 100}%` }} />
        </div>
        <div className={`trp__wave ${listening ? "is-on" : ""}`} aria-hidden="true">
          {[0, 1, 2, 3, 4].map((i) => (
            <span key={i} style={{ transform: `scaleY(${listening ? 0.25 + level * (0.6 + ((i * 7) % 5) / 6) : 0.15})` }} />
          ))}
        </div>
      </div>
      <div className="trp__mic">
        <select
          className="wp-input"
          value={mic}
          aria-label="Microphone"
          onChange={(e) => {
            setMic(e.target.value);
            void api.setSetting("speech_mic", e.target.value);
          }}
        >
          <option value="">System default{mics[0] ? ` (${mics[0]})` : ""}</option>
          {mics.map((m, i) => (
            <option key={m} value={m}>
              {m}
              {i === 0 ? " (default)" : ""}
            </option>
          ))}
        </select>
        <label className="trp__gain" title="Input gain: 50% leaves the sound as it is">
          Gain
          <input
            type="range"
            min={0}
            max={100}
            value={gain ?? 50}
            onChange={(e) => {
              const v = Number(e.target.value);
              setGain(v);
              void api.speechSetGain(v);
            }}
            aria-label="Input gain"
          />
          <span className="wp-num">{gain ?? 50}%</span>
        </label>
      </div>
      {s?.warning && <div className="trp__warn">{s.warning}</div>}
      {s?.idle_paused && !listening && (
        <div className="trp__warn trp__warn--info">
          Paused after 10 minutes without speech (the transcript is saved).
          <button className="wp-btn wp-btn--primary" onClick={onStart}>
            Resume
          </button>
        </div>
      )}
      {s?.problem && <p className="form-error">{s.problem}</p>}
      <nav className="trp__tabs">
        <button className={tab === "live" ? "is-on" : ""} onClick={() => setTab("live")}>
          Transcript
        </button>
        <button className={tab === "history" ? "is-on" : ""} onClick={() => setTab("history")}>
          History
        </button>
      </nav>
      {tab === "live" ? (
        <div className="trp__body">
          <div className="trp__lines" aria-live="polite">
            {lines.length === 0 && !interim && <p className="wp-note">{listening ? "Listening: the words appear here." : "Start transcribing to see what is said."}</p>}
            {lines.map(([t, text, c], i) => (
              <p key={i} className="trp__line">
                <span className={`trp__dot is-${conf(c)}`} title={confTip(c)} />
                <span className="trp__time wp-num">{t.slice(0, 5)}</span> {text}
              </p>
            ))}
            {interim && (
              <p className="trp__line trp__line--now">
                <span className="trp__dot is-none" />
                <span className="trp__time" /> {interim}
                <span className="trp__cursor" aria-hidden="true" />
              </p>
            )}
            {listening && !interim && lines.length > 0 && <span className="trp__cursor trp__cursor--alone" aria-hidden="true" />}
            <div ref={bottom} />
          </div>
          <div className="trp__det">
            <div className="trp__det-head">Scriptures heard</div>
            {(s?.recent ?? []).length === 0 && <p className="wp-note">Scriptures heard appear here.</p>}
            {(s?.recent ?? []).map((d) => (
              <div key={d.reference + d.at} className="trp__det-row">
                <div>
                  <strong>{d.reference}</strong>
                  <small>“{d.heard}”</small>
                </div>
                <button className="wp-btn wp-btn--quiet" onClick={() => onPreview(d.reference)}>
                  Preview
                </button>
                <button className="wp-btn wp-btn--primary" onClick={() => onLive(d.reference)}>
                  Go live
                </button>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <History />
      )}
    </aside>
  );
}

function History() {
  const [list, setList] = useState<{ name: string; bytes: number }[]>([]);
  const [open, setOpen] = useState<{ name: string; text: string } | null>(null);
  const load = () => api.speechHistory().then(setList).catch(() => {});
  useEffect(() => {
    void load();
  }, []);
  if (open)
    return (
      <div className="trp__body trp__body--col">
        <div className="trp__hist-head">
          <button className="btn-quiet" onClick={() => setOpen(null)}>
            ‹ All sessions
          </button>
          <strong>{open.name}</strong>
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--quiet" onClick={() => void navigator.clipboard.writeText(open.text).catch(() => {})}>
            Copy
          </button>
        </div>
        <pre className="trp__hist-text">{open.text}</pre>
      </div>
    );
  return (
    <div className="trp__body trp__body--col">
      {list.length === 0 && <p className="wp-note">Each session is saved here when you stop transcribing.</p>}
      {list.map((h) => (
        <div key={h.name} className="trp__hist-row">
          <button className="btn-quiet" onClick={() => void api.speechHistoryRead(h.name).then((text) => setOpen({ name: h.name, text }))}>
            {h.name}
          </button>
          <span className="wp-note wp-note--inline">{Math.max(1, Math.round(h.bytes / 1024))} KB</span>
          <button className="wp-icon-btn" aria-label={`Delete ${h.name}`} onClick={() => confirm(`Delete the transcript of ${h.name}?`) && void api.speechHistoryDelete(h.name).then(load)}>
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}
