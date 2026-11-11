import { useEffect, useRef, useState } from "react";
import { api, type CameraLive } from "./api";

// A camera on the main screen: choose the camera, see it here, then show it full
// screen, behind the words, or in a corner. Blank, Logo and held outputs still win.

export function CameraPanel({ live, onClose, say }: { live: CameraLive | null; onClose: () => void; say: (m: string, bad?: boolean) => void }) {
  const [cams, setCams] = useState<MediaDeviceInfo[]>([]);
  const [device, setDevice] = useState(live?.device_id ?? "");
  const [mode, setMode] = useState<CameraLive["mode"]>(live?.mode ?? "full");
  const [problem, setProblem] = useState<string | null>(null);
  const video = useRef<HTMLVideoElement>(null);

  // asking once lets the camera names show
  useEffect(() => {
    navigator.mediaDevices
      ?.getUserMedia({ video: true })
      .then((s) => s.getTracks().forEach((t) => t.stop()))
      .catch(() => {})
      .finally(() =>
        navigator.mediaDevices
          ?.enumerateDevices()
          .then((d) => {
            const v = d.filter((x) => x.kind === "videoinput");
            setCams(v);
            setDevice((cur) => cur || v[0]?.deviceId || "");
            if (!v.length) setProblem("No camera found. Plug in a USB camera or a capture card, then open this again.");
          })
          .catch(() => setProblem("Cameras cannot be listed on this computer.")),
      );
  }, []);

  // small preview here (never on the screens until Show is chosen)
  useEffect(() => {
    if (!device) return;
    let stream: MediaStream | null = null;
    let stop = false;
    navigator.mediaDevices
      ?.getUserMedia({ video: { deviceId: { exact: device } }, audio: false })
      .then((s) => {
        if (stop) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        if (video.current) video.current.srcObject = s;
        setProblem(null);
      })
      .catch(() => setProblem("That camera is busy or unplugged."));
    return () => {
      stop = true;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, [device]);

  const label = cams.find((c) => c.deviceId === device)?.label || "Camera";
  const show = async () => {
    try {
      await api.setCamera({ device_id: device, label, mode });
      say(`${label} is on the main screen (${mode === "full" ? "full screen" : mode === "behind" ? "behind the words" : "in the corner"}).`);
    } catch (e) {
      say(String(e instanceof Error ? e.message : e), true);
    }
  };
  return (
    <div className="popover popover--cue" role="dialog" aria-label="Camera">
      <div className="cue-now">
        <div className="cue-now__label">{live ? `On the screens: ${live.label}` : "Camera"}</div>
        <video ref={video} className="cam-preview" autoPlay muted playsInline />
        {problem && <p className="form-error">{problem}</p>}
      </div>
      <div className="form">
        <label>
          <span>Camera</span>
          <select value={device} onChange={(e) => setDevice(e.target.value)}>
            {cams.map((c, i) => (
              <option key={c.deviceId || i} value={c.deviceId}>
                {c.label || `Camera ${i + 1}`}
              </option>
            ))}
          </select>
        </label>
        <div className="cam-modes" role="radiogroup" aria-label="How to show it">
          {(
            [
              ["full", "Full screen"],
              ["behind", "Behind the words"],
              ["corner", "Corner"],
            ] as const
          ).map(([m, l]) => (
            <label key={m} className="check">
              <input type="radio" name="cam-mode" checked={mode === m} onChange={() => setMode(m)} /> <span>{l}</span>
            </label>
          ))}
        </div>
        <p className="wp-note wp-note--inline">Main screen only. Blank and Logo still black it out.</p>
      </div>
      <div className="row row--end">
        {live && (
          <button className="btn-quiet" onClick={() => api.setCamera(null).then(() => say("Camera taken off the screens."))}>
            Take off
          </button>
        )}
        <button className="btn-quiet" onClick={onClose}>
          Done
        </button>
        <button className="btn" disabled={!device} onClick={show}>
          {live ? "Update" : "Show on screens"}
        </button>
      </div>
    </div>
  );
}
