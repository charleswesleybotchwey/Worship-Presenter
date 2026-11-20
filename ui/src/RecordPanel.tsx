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
  // stream keys live in this computer's protected store (Windows data protection), never here
  const [saved, setSaved] = useState<Record<string, boolean>>({});
  const [typing, setTyping] = useState<Record<string, string>>({});
  const loadSaved = () => api.streamKeysSaved().then(setSaved).catch(() => {});
  useEffect(() => {
    // keys kept here by earlier versions: move them to the protected store, then forget them
    const old = p.dests.filter((d) => d.target.trim());
    Promise.all(old.map((d) => api.streamKeySet(d.kind, d.target)))
      .then(() => old.length && setP((x) => ({ ...x, dests: x.dests.map((d) => ({ ...d, target: "" })) })))
      .finally(loadSaved);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    localStorage.setItem("wp-record", JSON.stringify({ ...p, dests: p.dests.map((d) => ({ ...d, target: "" })) }));
  }, [p]);
  const saveKey = async (kind: string) => {
    const v = (typing[kind] ?? "").trim();
    if (!v) return;
    await api.streamKeySet(kind, v);
    setTyping((t) => ({ ...t, [kind]: "" }));
    void loadSaved();
  };
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
                  {d.on &&
                    (saved[d.kind] && typing[d.kind] === undefined ? (
                      <div className="rec__saved">
                        <span className="wp-note wp-note--inline">{d.kind === "custom" ? "Address saved" : "Key saved"} (kept protected on this computer)</span>
                        <button className="btn-quiet" disabled={running} onClick={() => setTyping((t) => ({ ...t, [d.kind]: "" }))}>
                          Replace
                        </button>
                        <button className="btn-quiet" disabled={running} onClick={() => void api.streamKeySet(d.kind, "").then(loadSaved)}>
                          Remove
                        </button>
                      </div>
                    ) : (
                      <input
                        className="wp-input"
                        type={d.kind === "custom" ? "text" : "password"}
                        autoComplete="off"
                        disabled={running}
                        placeholder={d.kind === "custom" ? "rtmp://your-relay/live/key (saved when you leave the box)" : "Paste the stream key (saved when you leave the box)"}
                        value={typing[d.kind] ?? ""}
                        onChange={(e) => setTyping((t) => ({ ...t, [d.kind]: e.target.value }))}
                        onBlur={() => void saveKey(d.kind)}
                        aria-label={NAMES[d.kind]}
                      />
                    ))}
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
                    onClick={() => void act(() => api.streamStart({ record: p.record, folder: p.folder || null, destinations: p.dests.filter((d) => d.on).map(({ kind }) => ({ kind, target: "" })) }))}
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
