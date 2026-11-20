import { useEffect, useRef, useState } from "react";
import { api } from "./api";
import { STAGE_PARTS, STAGE_PRESETS, StageView, type StageBox, type StageLayout, type StagePart } from "./StageView";

// Settings → Stage display: the standard stage screen, or a custom one made of boxes
// (words now, next slide, clock, timer, item, messages, key) placed by dragging.

const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));

export function StageLayoutEditor() {
  const [draft, setDraft] = useState<StageLayout | null>(null);
  const [saved, setSaved] = useState<string>("");
  const [sel, setSel] = useState<number | null>(null);
  const area = useRef<HTMLDivElement>(null);
  useEffect(() => {
    api
      .getSettings()
      .then((x) => {
        setSaved(x.stage_layout ?? "");
        try {
          const v = x.stage_layout ? (JSON.parse(x.stage_layout) as StageLayout) : null;
          setDraft(v?.boxes?.length ? v : null);
        } catch {
          setDraft(null);
        }
      })
      .catch(() => {});
  }, []);
  const custom = draft != null;
  const set = (i: number, p: Partial<StageBox>) => setDraft((d) => d && { boxes: d.boxes.map((b, k) => (k === i ? { ...b, ...p } : b)) });
  // drag to move, drag the corner to resize (in % of the preview)
  const drag = (e: React.PointerEvent, i: number, resize: boolean) => {
    e.preventDefault();
    e.stopPropagation();
    setSel(i);
    const r = area.current!.getBoundingClientRect();
    const start = { x: e.clientX, y: e.clientY, b: draft!.boxes[i] };
    const move = (m: PointerEvent) => {
      const dx = ((m.clientX - start.x) / r.width) * 100;
      const dy = ((m.clientY - start.y) / r.height) * 100;
      const b = start.b;
      if (resize) set(i, { w: Math.round(clamp(b.w + dx, 4, 100 - b.x)), h: Math.round(clamp(b.h + dy, 3, 100 - b.y)) });
      else set(i, { x: Math.round(clamp(b.x + dx, 0, 100 - b.w)), y: Math.round(clamp(b.y + dy, 0, 100 - b.h)) });
    };
    const up = () => (window.removeEventListener("pointermove", move), window.removeEventListener("pointerup", up));
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  const json = draft ? JSON.stringify(draft) : "";
  const save = () => void api.setSetting("stage_layout", json).then(() => setSaved(json));
  const b = sel != null ? draft?.boxes[sel] : undefined;
  return (
    <section className="sle">
      <h2>Stage display</h2>
      <p className="wp-note">What the pulpit and the band see on the stage (foldback) screen. Changes show on stage screens as soon as you save.</p>
      <div className="sle__mode">
        <label className="wp-check">
          <input type="radio" checked={!custom} onChange={() => (setDraft(null), setSel(null))} /> Standard
        </label>
        <label className="wp-check">
          <input type="radio" checked={custom} onChange={() => setDraft(STAGE_PRESETS.words.layout)} /> Custom
        </label>
        {custom && (
          <select className="wp-input" value="" onChange={(e) => e.target.value && (setDraft(e.target.value === "empty" ? { boxes: [] } : STAGE_PRESETS[e.target.value].layout), setSel(null))} aria-label="Start from">
            <option value="">Start from…</option>
            {Object.entries(STAGE_PRESETS).map(([k, p]) => (
              <option key={k} value={k}>
                {p.label}
              </option>
            ))}
            <option value="empty">An empty screen</option>
          </select>
        )}
      </div>
      <div className="sle__body">
        <div className="sle__screen" ref={area} onPointerDown={() => setSel(null)}>
          <StageView b={null} lost={false} layout={draft} />
          {draft?.boxes.map((bx, i) => (
            <div
              key={i}
              className={`sle__box ${sel === i ? "is-on" : ""}`}
              style={{ left: `${bx.x}%`, top: `${bx.y}%`, width: `${bx.w}%`, height: `${bx.h}%` }}
              onPointerDown={(e) => drag(e, i, false)}
              title="Drag to move"
            >
              <span className="sle__tag">{STAGE_PARTS.find((p) => p.part === bx.part)?.label}</span>
              <span className="sle__corner" onPointerDown={(e) => drag(e, i, true)} title="Drag to resize" />
            </div>
          ))}
        </div>
        {custom && (
          <div className="sle__side">
            <select className="wp-input" value="" onChange={(e) => e.target.value && (setDraft((d) => d && { boxes: [...d.boxes, { part: e.target.value as StagePart, x: 30, y: 40, w: 40, h: 15, size: 5, color: "#ffffff", bg: "transparent", align: "left" }] }), setSel(draft!.boxes.length))} aria-label="Add a box">
              <option value="">+ Add a box…</option>
              {STAGE_PARTS.map((p) => (
                <option key={p.part} value={p.part}>
                  {p.label}
                </option>
              ))}
            </select>
            {b && sel != null ? (
              <div className="sle__props">
                <label className="ps__row">
                  <span>Shows</span>
                  <select className="wp-input" value={b.part} onChange={(e) => set(sel, { part: e.target.value as StagePart })} aria-label="Shows">
                    {STAGE_PARTS.map((p) => (
                      <option key={p.part} value={p.part}>
                        {p.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="ps__row">
                  <span>Text size</span>
                  <input className="wp-input" type="number" min={1} max={40} step={0.5} value={b.size} onChange={(e) => set(sel, { size: Number(e.target.value) || 5 })} aria-label="Text size" />
                </label>
                <label className="ps__row">
                  <span>Colour</span>
                  <input type="color" value={b.color} onChange={(e) => set(sel, { color: e.target.value })} aria-label="Colour" />
                </label>
                <label className="ps__row">
                  <span>Background</span>
                  <input type="color" value={b.bg === "transparent" ? "#000000" : b.bg} onChange={(e) => set(sel, { bg: e.target.value })} aria-label="Background" />
                  <button className="btn-quiet" onClick={() => set(sel, { bg: "transparent" })}>
                    None
                  </button>
                </label>
                <label className="ps__row">
                  <span>Align</span>
                  <select className="wp-input" value={b.align} onChange={(e) => set(sel, { align: e.target.value as StageBox["align"] })} aria-label="Align">
                    <option value="left">Left</option>
                    <option value="center">Centre</option>
                    <option value="right">Right</option>
                  </select>
                </label>
                <button className="btn-quiet btn-danger" onClick={() => (setDraft((d) => d && { boxes: d.boxes.filter((_, k) => k !== sel) }), setSel(null))}>
                  Remove this box
                </button>
              </div>
            ) : (
              <p className="wp-note">Click a box to change it. Drag it to move it; drag its corner to resize it.</p>
            )}
          </div>
        )}
      </div>
      <div className="ps__foot">
        <button className="wp-btn wp-btn--primary" disabled={json === saved} onClick={save}>
          {json === saved ? "Saved" : "Save"}
        </button>
      </div>
    </section>
  );
}
