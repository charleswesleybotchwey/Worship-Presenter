import { useEffect, useState } from "react";
import { api, onPropsChanged, type MediaItem, type Prop } from "./api";

// Props (as ProPresenter): graphics shown independently of the slides — switched on and
// off here, made and changed here.

const BLANK: Prop = { id: null, name: "", kind: "text", text: "", media_id: null, position: "bottom-right", size: 4, opacity: 1, color: "#ffffff", background: "#000000aa", on: "all" };
const PLACES: [Prop["position"], string][] = [
  ["top-left", "Top left"],
  ["top-right", "Top right"],
  ["bottom-left", "Bottom left"],
  ["bottom-right", "Bottom right"],
  ["top", "Band across the top"],
  ["bottom", "Band across the bottom"],
  ["centre", "Centre"],
];

export function PropsPanel({ showing, onClose }: { showing: number[]; onClose: () => void }) {
  const [all, setAll] = useState<Prop[]>([]);
  const [edit, setEdit] = useState<Prop | null>(null);
  const [pictures, setPictures] = useState<MediaItem[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const load = () => api.propsList().then(setAll).catch(() => {});
  useEffect(() => {
    void load();
    let off = () => {};
    onPropsChanged(() => void load()).then((u) => (off = u));
    Promise.all([api.listMedia("image", ""), api.listMedia("video", "")])
      .then(([a, b]) => setPictures([...a, ...b]))
      .catch(() => {});
    return () => off();
  }, []);
  const toggle = (id: number) => void api.setProps(showing.includes(id) ? showing.filter((x) => x !== id) : [...showing, id]);
  const save = async () => {
    if (!edit) return;
    try {
      await api.propSave(edit);
      setEdit(null);
      setProblem(null);
      void load();
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };
  return (
    <aside className="opn prp" role="dialog" aria-label="Props">
      <header className="opn__head">
        <h2>Props</h2>
        <button className="wp-btn wp-btn--quiet" onClick={onClose}>
          Close
        </button>
      </header>
      <p className="wp-note">Graphics that stay on screen whatever the slides do: a logo, a LIVE badge, the sermon title, a clock.</p>
      <div className="prp__list">
        {all.length === 0 && <p className="wp-note">No props yet.</p>}
        {all.map((p) => {
          const on = p.id != null && showing.includes(p.id);
          return (
            <div key={p.id} className={`prp__row ${on ? "is-on" : ""}`}>
              <button className={`wp-btn wp-mode ${on ? "is-on is-on--clear" : ""}`} aria-pressed={on} onClick={() => p.id != null && toggle(p.id)}>
                {on && <span className="wp-mode__dot" aria-hidden="true" />}
                {p.name}
              </button>
              <small className="wp-note">{p.on === "stream" ? "Stream" : p.on === "screens" ? "Screens" : "Screens and stream"}</small>
              <button className="btn-quiet" onClick={() => setEdit({ ...p })}>
                Edit
              </button>
            </div>
          );
        })}
      </div>
      <div className="prp__foot">
        <button className="wp-btn" disabled={!showing.length} onClick={() => void api.setProps([])}>
          Clear props
        </button>
        <button className="wp-btn wp-btn--primary" onClick={() => setEdit({ ...BLANK })}>
          New prop
        </button>
      </div>
      {edit && (
        <div className="prp__edit">
          <h3>{edit.id == null ? "New prop" : `Edit “${edit.name}”`}</h3>
          <label className="ps__row">
            <span>Name</span>
            <input className="wp-input" value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.target.value })} aria-label="Prop name" autoFocus />
          </label>
          <label className="ps__row">
            <span>Shows</span>
            <select className="wp-input" value={edit.kind} onChange={(e) => setEdit({ ...edit, kind: e.target.value as Prop["kind"] })} aria-label="Prop kind">
              <option value="text">Text</option>
              <option value="image">A picture or looping video</option>
              <option value="clock">The time</option>
            </select>
          </label>
          {edit.kind === "text" && (
            <label className="ps__row">
              <span>Text</span>
              <input className="wp-input" value={edit.text} onChange={(e) => setEdit({ ...edit, text: e.target.value })} aria-label="Prop text" />
            </label>
          )}
          {edit.kind === "image" && (
            <label className="ps__row">
              <span>Picture</span>
              <select className="wp-input" value={edit.media_id ?? ""} onChange={(e) => setEdit({ ...edit, media_id: e.target.value ? Number(e.target.value) : null })} aria-label="Prop picture">
                <option value="">Choose a picture or video from the media library…</option>
                {pictures.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.kind === "video" ? "🎞 " : ""}
                    {m.title}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="ps__row">
            <span>Place</span>
            <select className="wp-input" value={edit.position} onChange={(e) => setEdit({ ...edit, position: e.target.value as Prop["position"] })} aria-label="Prop place">
              {PLACES.map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
          </label>
          <label className="ps__row">
            <span>{edit.kind === "image" ? "Width (%)" : "Text size (%)"}</span>
            <input className="wp-input" type="number" min={1} max={100} step={0.5} value={edit.size} onChange={(e) => setEdit({ ...edit, size: Number(e.target.value) || 4 })} aria-label="Prop size" />
          </label>
          <label className="ps__row">
            <span>Opacity</span>
            <input type="range" min={0.1} max={1} step={0.05} value={edit.opacity} onChange={(e) => setEdit({ ...edit, opacity: Number(e.target.value) })} aria-label="Prop opacity" />
          </label>
          {edit.kind !== "image" && (
            <label className="ps__row">
              <span>Colours</span>
              <input type="color" value={edit.color} onChange={(e) => setEdit({ ...edit, color: e.target.value })} aria-label="Text colour" />
              <input type="color" value={(edit.background || "#000000").slice(0, 7)} onChange={(e) => setEdit({ ...edit, background: e.target.value + "cc" })} aria-label="Band colour" />
              <button className="btn-quiet" onClick={() => setEdit({ ...edit, background: "" })}>
                No band
              </button>
            </label>
          )}
          <label className="ps__row">
            <span>On</span>
            <select className="wp-input" value={edit.on} onChange={(e) => setEdit({ ...edit, on: e.target.value as Prop["on"] })} aria-label="Prop on">
              <option value="all">The screens and the stream</option>
              <option value="screens">The screens only</option>
              <option value="stream">The stream (lower third) only</option>
            </select>
          </label>
          {problem && <p className="form-error">{problem}</p>}
          <div className="ps__foot">
            {edit.id != null && (
              <button className="btn-quiet btn-danger" onClick={() => confirm(`Delete the prop “${edit.name}”?`) && void api.propDelete(edit.id!).then(() => (setEdit(null), load()))}>
                Delete
              </button>
            )}
            <button className="wp-btn" onClick={() => setEdit(null)}>
              Cancel
            </button>
            <button className="wp-btn wp-btn--primary" onClick={() => void save()}>
              Save
            </button>
          </div>
        </div>
      )}
    </aside>
  );
}
