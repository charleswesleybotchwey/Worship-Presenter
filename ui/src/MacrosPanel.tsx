import { useEffect, useState } from "react";
import { api, onMacrosChanged, type Macro, type MacroAction, type OutputConfig, type Prop } from "./api";

// Macros (as ProPresenter): one button that runs several actions in order.

export const ACTIONS: [string, string][] = [
  ["blank", "Blank"],
  ["logo", "Logo"],
  ["clear", "Clear the words"],
  ["resume", "Resume (show the slide)"],
  ["media_off", "Media off"],
  ["media_on", "Media on"],
  ["prop_on", "Show a prop"],
  ["prop_off", "Hide a prop"],
  ["props_clear", "Clear props"],
  ["next_item", "Next item"],
  ["previous_item", "Previous item"],
  ["countdown", "Start a countdown (minutes)"],
  ["timer_stop", "Stop the timer"],
  ["music_play", "Play the background music"],
  ["music_stop", "Stop the music"],
  ["look", "Change a screen's look"],
  ["clear_all", "Clear all"],
  ["wait", "Wait (seconds)"],
];
const NAME = Object.fromEntries(ACTIONS);
const LOOKS: [string, string][] = [
  ["full", "Full slides"],
  ["lower", "Lower third"],
  ["stage", "Stage display"],
  ["words", "Words only"],
  ["media", "Media only"],
];

export function MacrosPanel({ onRun, onClose }: { onRun: (m: Macro) => void; onClose: () => void }) {
  const [all, setAll] = useState<Macro[]>([]);
  const [edit, setEdit] = useState<Macro | null>(null);
  const [props, setProps] = useState<Prop[]>([]);
  const [screens, setScreens] = useState<OutputConfig[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const load = () => api.macrosList().then(setAll).catch(() => {});
  useEffect(() => {
    void load();
    let off = () => {};
    onMacrosChanged(() => void load()).then((u) => (off = u));
    api.propsList().then(setProps).catch(() => {});
    api.outputConfigs().then(setScreens).catch(() => {});
    return () => off();
  }, []);
  const setStep = (i: number, a: MacroAction) => edit && setEdit({ ...edit, actions: edit.actions.map((x, k) => (k === i ? a : x)) });
  const move = (i: number, d: number) => {
    if (!edit) return;
    const l = [...edit.actions];
    const j = i + d;
    if (j < 0 || j >= l.length) return;
    [l[i], l[j]] = [l[j], l[i]];
    setEdit({ ...edit, actions: l });
  };
  const value = (a: MacroAction, i: number) => {
    if (a.kind === "prop_on" || a.kind === "prop_off")
      return (
        <select className="wp-input" value={Number(a.value) || ""} onChange={(e) => setStep(i, { ...a, value: Number(e.target.value) })} aria-label="Which prop">
          <option value="">Choose…</option>
          {props.map((p) => (
            <option key={p.id} value={p.id ?? ""}>
              {p.name}
            </option>
          ))}
        </select>
      );
    if (a.kind === "countdown" || a.kind === "wait")
      return <input className="wp-input mac__num" type="number" min={1} max={a.kind === "wait" ? 60 : 180} value={Number(a.value) || (a.kind === "wait" ? 2 : 5)} onChange={(e) => setStep(i, { ...a, value: Number(e.target.value) })} aria-label={a.kind === "wait" ? "Seconds" : "Minutes"} />;
    if (a.kind === "look") {
      const v = (a.value as { role?: string; look?: string }) ?? {};
      return (
        <span className="mac__pair">
          <select className="wp-input" value={v.role ?? ""} onChange={(e) => setStep(i, { ...a, value: { ...v, role: e.target.value } })} aria-label="Which screen">
            <option value="">Screen…</option>
            {screens.map((o) => (
              <option key={o.role} value={o.role}>
                {o.name}
              </option>
            ))}
          </select>
          <select className="wp-input" value={v.look ?? "full"} onChange={(e) => setStep(i, { ...a, value: { ...v, look: e.target.value } })} aria-label="Look">
            {LOOKS.map(([k, l]) => (
              <option key={k} value={k}>
                {l}
              </option>
            ))}
          </select>
        </span>
      );
    }
    return null;
  };
  return (
    <aside className="opn mac" role="dialog" aria-label="Macros">
      <header className="opn__head">
        <h2>Macros</h2>
        <button className="wp-btn wp-btn--quiet" onClick={onClose}>
          Close
        </button>
      </header>
      <p className="wp-note">One button for several actions: the walk-in, the sermon, the end of the service. Stream Deck: Companion address /api/v1/macro/N.</p>
      <div className="mac__grid">
        {all.length === 0 && <p className="wp-note">No macros yet.</p>}
        {all.map((m) => (
          <div key={m.id} className="mac__tile">
            <button className="mac__run" style={{ "--grp": m.color } as React.CSSProperties} onClick={() => onRun(m)} title={m.actions.map((a) => NAME[a.kind] ?? a.kind).join(" → ")}>
              {m.name}
            </button>
            <button className="btn-quiet" onClick={() => setEdit(structuredClone(m))}>
              Edit · #{m.id}
            </button>
          </div>
        ))}
      </div>
      <button className="wp-btn wp-btn--primary" onClick={() => setEdit({ id: null, name: "", color: "#3b82f6", actions: [{ kind: "logo" }] })}>
        New macro
      </button>
      {edit && (
        <div className="prp__edit">
          <h3>{edit.id == null ? "New macro" : `Edit “${edit.name}”`}</h3>
          <label className="ps__row">
            <span>Name</span>
            <input className="wp-input" value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.target.value })} aria-label="Macro name" autoFocus />
            <input type="color" value={edit.color} onChange={(e) => setEdit({ ...edit, color: e.target.value })} aria-label="Macro colour" />
          </label>
          <ol className="mac__steps">
            {edit.actions.map((a, i) => (
              <li key={i} className="mac__step">
                <select className="wp-input" value={a.kind} onChange={(e) => setStep(i, { kind: e.target.value })} aria-label={`Step ${i + 1}`}>
                  {ACTIONS.map(([k, l]) => (
                    <option key={k} value={k}>
                      {l}
                    </option>
                  ))}
                </select>
                {value(a, i)}
                <span className="mac__moves">
                  <button className="btn-quiet" aria-label="Move up" onClick={() => move(i, -1)}>
                    ↑
                  </button>
                  <button className="btn-quiet" aria-label="Move down" onClick={() => move(i, 1)}>
                    ↓
                  </button>
                  <button className="btn-quiet" aria-label="Remove step" onClick={() => setEdit({ ...edit, actions: edit.actions.filter((_, k) => k !== i) })}>
                    ×
                  </button>
                </span>
              </li>
            ))}
          </ol>
          <button className="btn-quiet" onClick={() => setEdit({ ...edit, actions: [...edit.actions, { kind: "wait", value: 2 }] })}>
            + Add a step
          </button>
          {problem && <p className="form-error">{problem}</p>}
          <div className="ps__foot">
            {edit.id != null && (
              <button className="btn-quiet btn-danger" onClick={() => confirm(`Delete the macro “${edit.name}”?`) && void api.macroDelete(edit.id!).then(() => (setEdit(null), load()))}>
                Delete
              </button>
            )}
            <button className="wp-btn" onClick={() => setEdit(null)}>
              Cancel
            </button>
            <button
              className="wp-btn wp-btn--primary"
              onClick={() =>
                void api.macroSave(edit).then(
                  () => (setEdit(null), setProblem(null), load()),
                  (e) => setProblem(String(e instanceof Error ? e.message : e)),
                )
              }
            >
              Save
            </button>
          </div>
        </div>
      )}
    </aside>
  );
}
