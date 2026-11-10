import { useEffect, useState } from "react";
import { api, type Alert, type AlertPreset, type LiveState, type TimerPreset, type TimerShow } from "./api";
import { fmtLeft, timerLeft, useNow } from "./Cues";

// Timer and Alert panels opened from the top bar.

const SHOW: { value: TimerShow; label: string }[] = [
  { value: "off", label: "Not on screens" },
  { value: "corner", label: "Corner" },
  { value: "full", label: "Full screen" },
];

function useRun() {
  const [problem, setProblem] = useState<string | null>(null);
  const run = async (f: () => Promise<unknown>) => {
    try {
      await f();
      setProblem(null);
      return true;
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
      return false;
    }
  };
  return { problem, run };
}

export function TimerPanel({ state, onClose }: { state: LiveState | undefined; onClose: () => void }) {
  const [presets, setPresets] = useState<TimerPreset[]>([]);
  const [label, setLabel] = useState("");
  const [mode, setMode] = useState<TimerPreset["mode"]>("countdown");
  const [value, setValue] = useState("5:00");
  const [show, setShow] = useState<TimerShow>("corner");
  const [stage, setStage] = useState(true);
  const { problem, run } = useRun();
  const t = state?.timer ?? null;
  const now = useNow(!!t);

  const load = () => api.cuePresets().then((p) => setPresets(p.timers));
  useEffect(() => void load(), []);
  const start = (l: string, m: TimerPreset["mode"], v: string) => run(() => api.timerStart(l, m, v, show, stage));

  return (
    <div className="popover popover--cue" role="dialog" aria-label="Timer">
      {t && (
        <div className="cue-now">
          <div className={`cue-now__time ${timerLeft(t, now) < 0 ? "is-over" : ""}`}>{fmtLeft(timerLeft(t, now))}</div>
          <div className="cue-now__label">{t.label || "Timer"}{t.paused_left_ms != null ? ", paused" : ""}</div>
          <div className="row">
            {t.paused_left_ms != null ? (
              <button className="btn" onClick={() => run(() => api.timerControl("resume"))}>
                Resume
              </button>
            ) : (
              <button className="btn" onClick={() => run(() => api.timerControl("pause"))}>
                Pause
              </button>
            )}
            <button className="btn-quiet" onClick={() => run(() => api.timerControl("add", { ms: 60_000 }))}>
              +1 min
            </button>
            <button className="btn-quiet" onClick={() => run(() => api.timerControl("add", { ms: -60_000 }))}>
              −1 min
            </button>
            <button className="btn-quiet btn-danger" onClick={() => run(() => api.timerControl("stop"))}>
              Stop
            </button>
          </div>
          <div className="row">
            <div className="segmented" role="radiogroup" aria-label="On the screens">
              {SHOW.map((o) => (
                <button key={o.value} role="radio" aria-checked={t.on_screens === o.value} onClick={() => run(() => api.timerControl("show", { onScreens: o.value }))}>
                  {o.label}
                </button>
              ))}
            </div>
            <label className="check-inline">
              <input type="checkbox" checked={t.on_stage} onChange={(e) => run(() => api.timerControl("show", { onStage: e.target.checked }))} /> Stage
            </label>
          </div>
        </div>
      )}
      <h3 className="cue-h">Start a timer</h3>
      <ul className="menu-list">
        {presets.map((p) => (
          <li key={p.id}>
            <button className="menu-item" onClick={() => start(p.name, p.mode, p.value)}>
              <span>{p.name}</span>
              <span className="menu-item__meta">{p.mode === "countdown" ? p.value : `until ${p.value}`}</span>
              <span className="menu-item__meta">Start</span>
            </button>
            <button className="icon-btn" title="Delete" aria-label={`Delete ${p.name}`} onClick={() => p.id != null && api.deleteCuePreset("timer", p.id).then(load)}>
              ×
            </button>
          </li>
        ))}
      </ul>
      <div className="form">
        <label>
          <span>Name</span>
          <input value={label} placeholder="Shown above the time (optional)" onChange={(e) => setLabel(e.target.value)} />
        </label>
        <label>
          <span>Count</span>
          <select value={mode} onChange={(e) => (setMode(e.target.value as TimerPreset["mode"]), setValue(e.target.value === "countdown" ? "5:00" : "09:00"))}>
            <option value="countdown">Down from a length</option>
            <option value="to_time">Down to a time of day</option>
          </select>
        </label>
        <label>
          <span>{mode === "countdown" ? "Length" : "Time"}</span>
          <input value={value} placeholder={mode === "countdown" ? "5:00" : "09:00 or 9:30 am"} onChange={(e) => setValue(e.target.value)} />
        </label>
        <label>
          <span>Screens</span>
          <select value={show} onChange={(e) => setShow(e.target.value as TimerShow)}>
            {SHOW.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
        <label className="check">
          <input type="checkbox" checked={stage} onChange={(e) => setStage(e.target.checked)} />
          <span>Show on the stage display</span>
        </label>
        <div className="row row--end">
          <button className="btn-quiet" onClick={() => run(() => api.saveTimerPreset({ id: null, name: label.trim() || (mode === "countdown" ? `${value} countdown` : `Until ${value}`), mode, value }).then(load))}>
            Save as preset
          </button>
          <button className="btn" onClick={() => start(label, mode, value)}>
            Start
          </button>
        </div>
      </div>
      {problem && <p className="form-error">{problem}</p>}
      <div className="row row--end">
        <button className="btn-quiet" onClick={onClose}>
          Done
        </button>
      </div>
    </div>
  );
}

const SECONDS = [
  { value: 0, label: "Until I clear it" },
  { value: 30, label: "30 seconds" },
  { value: 45, label: "45 seconds" },
  { value: 60, label: "1 minute" },
  { value: 120, label: "2 minutes" },
];

export function AlertPanel({ alert, onClose }: { alert: Alert | null | undefined; onClose: () => void }) {
  const [presets, setPresets] = useState<AlertPreset[]>([]);
  const [text, setText] = useState("");
  const [values, setValues] = useState<Record<string, string>>({});
  const [style, setStyle] = useState<Alert["style"]>("band");
  const [position, setPosition] = useState<Alert["position"]>("bottom");
  const [seconds, setSeconds] = useState(45);
  const [where, setWhere] = useState({ screens: true, stream: false, stage: true });
  const { problem, run } = useRun();

  const load = () => api.cuePresets().then((p) => setPresets(p.alerts));
  useEffect(() => void load(), []);
  const blanks = [...new Set([...text.matchAll(/\{([^}]+)\}/g)].map((m) => m[1].trim()))];
  const pick = (p: AlertPreset) => {
    setText(p.text);
    setStyle(p.style);
    setPosition(p.position);
    setSeconds(p.seconds ?? 0);
    setValues({});
  };
  const show = () =>
    run(() =>
      api.alertShow({
        text,
        values: blanks.map((b) => [b, values[b] ?? ""] as [string, string]),
        style,
        position,
        seconds: seconds || null,
        onScreens: where.screens,
        onStream: where.stream,
        onStage: where.stage,
      }),
    );

  return (
    <div className="popover popover--cue" role="dialog" aria-label="Alert">
      {alert && (
        <div className="cue-now">
          <div className="cue-now__label">Showing now</div>
          <div className="cue-now__text">{alert.text}</div>
          <div className="row">
            <button className="btn" onClick={() => run(() => api.alertClear())}>
              Clear alert
            </button>
          </div>
        </div>
      )}
      <h3 className="cue-h">Saved alerts</h3>
      <ul className="menu-list">
        {presets.map((p) => (
          <li key={p.id} className={p.text === text ? "is-current" : ""}>
            <button className="menu-item menu-item--wrap" onClick={() => pick(p)}>
              <span>{p.text}</span>
            </button>
            <button className="icon-btn" title="Delete" aria-label="Delete this alert" onClick={() => p.id != null && api.deleteCuePreset("alert", p.id).then(load)}>
              ×
            </button>
          </li>
        ))}
      </ul>
      <div className="form">
        <label className="form__block">
          <span>Text. Put blanks in braces, like {"{number}"}.</span>
          <textarea rows={2} value={text} onChange={(e) => setText(e.target.value)} placeholder="Parent of child number {number}, please come to the children's room." />
        </label>
        {blanks.map((b) => (
          <label key={b}>
            <span>{b}</span>
            <input value={values[b] ?? ""} onChange={(e) => setValues({ ...values, [b]: e.target.value })} autoFocus={b === blanks[0]} onKeyDown={(e) => e.key === "Enter" && show()} />
          </label>
        ))}
        <label>
          <span>Style</span>
          <select value={`${style}-${position}`} onChange={(e) => (setStyle(e.target.value.split("-")[0] as Alert["style"]), setPosition(e.target.value.split("-")[1] as Alert["position"]))}>
            <option value="band-bottom">Band at the bottom</option>
            <option value="band-top">Band at the top</option>
            <option value="ticker-bottom">Scrolling at the bottom</option>
            <option value="ticker-top">Scrolling at the top</option>
          </select>
        </label>
        <label>
          <span>For</span>
          <select value={seconds} onChange={(e) => setSeconds(Number(e.target.value))}>
            {SECONDS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
        <div className="checks">
          <label className="check-inline">
            <input type="checkbox" checked={where.screens} onChange={(e) => setWhere({ ...where, screens: e.target.checked })} /> Projector
          </label>
          <label className="check-inline">
            <input type="checkbox" checked={where.stream} onChange={(e) => setWhere({ ...where, stream: e.target.checked })} /> Stream (lower third)
          </label>
          <label className="check-inline">
            <input type="checkbox" checked={where.stage} onChange={(e) => setWhere({ ...where, stage: e.target.checked })} /> Stage
          </label>
        </div>
        <div className="row row--end">
          <button className="btn-quiet" disabled={!text.trim()} onClick={() => run(() => api.saveAlertPreset({ id: null, text, style, position, seconds: seconds || null }).then(load))}>
            Save for next time
          </button>
          <button className="btn" disabled={!text.trim()} onClick={show}>
            Show alert
          </button>
        </div>
      </div>
      {problem && <p className="form-error">{problem}</p>}
      <div className="row row--end">
        <button className="btn-quiet" onClick={onClose}>
          Done
        </button>
      </div>
    </div>
  );
}
