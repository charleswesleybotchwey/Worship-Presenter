import { useEffect, useState } from "react";
import { api } from "./api";

// Recording the service and going live online (YouTube, Facebook, StreamRelay or any
// RTMP address): the main output and the computer's sound, through FFmpeg.

type Dest = { kind: "youtube" | "facebook" | "custom"; target: string; on: boolean };
const load = (): { record: boolean; folder: string; dests: Dest[] } => {
  try {
    return { record: true, folder: "", dests: [{ kind: "youtube", target: "", on: false }, { kind: "facebook", target: "", on: false }, { kind: "custom", target: "", on: false }], ...JSON.parse(localStorage.getItem("wp-record") || "{}") };
  } catch {
    return { record: true, folder: "", dests: [{ kind: "youtube", target: "", on: false }, { kind: "facebook", target: "", on: false }, { kind: "custom", target: "", on: false }] };
  }
};
const NAMES = { youtube: "YouTube (stream key)", facebook: "Facebook (stream key)", custom: "StreamRelay or other (rtmp:// address)" };

export type RecStatus = Awaited<ReturnType<typeof api.streamStatus>>;

export function RecordPanel({ status, onClose, refresh }: { status: RecStatus | null; onClose: () => void; refresh: () => void }) {
  const [p, setP] = useState(load);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  useEffect(() => localStorage.setItem("wp-record", JSON.stringify(p)), [p]);
  const act = async (f: () => Promise<unknown>) => {
    try {
      const r = await f();
      if (typeof r === "string") setMsg({ text: r });
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
    refresh();
  };
  const running = !!status?.running;
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog rec" role="dialog" aria-modal="true" aria-label="Record and go live online">
        <header className="wp-dialog__head">
          <h2>Record and go live online</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        <div className="ps__body">
          <p className="wp-note">The main output (exactly as the projector shows it) and this computer's sound. Streaming needs internet; recording does not.</p>
          {!status?.ffmpeg ? (
            <div className="ps__row">
              <span>FFmpeg</span>
              <span className="wp-note wp-note--inline">{status?.installing ?? "Needed once (about 100 MB), the free video encoder."}</span>
              <button className="wp-btn" disabled={!!status?.installing} onClick={() => void act(() => api.streamInstallFfmpeg())}>
                Download FFmpeg
              </button>
            </div>
          ) : (
            <>
              <label className="wp-check">
                <input type="checkbox" disabled={running} checked={p.record} onChange={(e) => setP({ ...p, record: e.target.checked })} /> Record to a video file (Videos\\Worship Presenter)
              </label>
              {p.dests.map((d, i) => (
                <div key={d.kind} className="rec__dest">
                  <label className="wp-check">
                    <input type="checkbox" disabled={running} checked={d.on} onChange={(e) => setP({ ...p, dests: p.dests.map((x, j) => (j === i ? { ...x, on: e.target.checked } : x)) })} /> {NAMES[d.kind]}
                  </label>
                  {d.on && (
                    <input
                      className="wp-input"
                      type={d.kind === "custom" ? "text" : "password"}
                      disabled={running}
                      placeholder={d.kind === "custom" ? "rtmp://your-relay/live/key" : "Paste the stream key"}
                      value={d.target}
                      onChange={(e) => setP({ ...p, dests: p.dests.map((x, j) => (j === i ? { ...x, target: e.target.value } : x)) })}
                      aria-label={NAMES[d.kind]}
                    />
                  )}
                </div>
              ))}
              {running ? (
                <div className="rec__live">
                  <span className="rec__dot" aria-hidden="true" />
                  <strong>
                    {Math.floor(status!.seconds / 3600)}:{String(Math.floor((status!.seconds % 3600) / 60)).padStart(2, "0")}:{String(status!.seconds % 60).padStart(2, "0")}
                  </strong>
                  <span className="wp-note wp-note--inline">{status!.report}</span>
                  <span className="wp-spacer" />
                  <button className="wp-btn" onClick={() => void act(async () => { const f = await api.streamStop(); return f ? `Saved: ${f}` : "Stopped."; })}>
                    Stop
                  </button>
                </div>
              ) : (
                <div className="ps__foot">
                  <button
                    className="wp-btn wp-btn--primary"
                    onClick={() => void act(() => api.streamStart({ record: p.record, folder: p.folder || null, destinations: p.dests.filter((d) => d.on).map(({ kind, target }) => ({ kind, target })) }))}
                  >
                    Start
                  </button>
                </div>
              )}
            </>
          )}
          {status?.report?.startsWith("Stopped") && <p className="form-error">{status.report}</p>}
          {msg && <p className={msg.bad ? "form-error" : "wp-note"}>{msg.text}</p>}
        </div>
      </div>
    </div>
  );
}
