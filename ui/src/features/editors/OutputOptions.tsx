import { useEffect, useState } from "react";
import { api, type Display, type OutputConfig } from "../../api";
import type { ThemeRow } from "../../theme/types";
import { ROLE_NAMES, outputLabel, type OutputView } from "../../live/outputs";

// EasyWorship's Options dialog for the outputs: Main, Alternate (lower third) and
// Foldback (stage), each with General, Themes and Logo & Blank settings.

type Tab = "general" | "themes" | "modes";

function Num({ label, value, onChange }: { label: string; value: number; onChange: (n: number) => void }) {
  return (
    <label className="oo-num">
      <span>{label}</span>
      <input type="number" value={value} onChange={(e) => onChange(Number(e.target.value) || 0)} />
    </label>
  );
}

export function OutputOptions({ views, mainLive }: { views: (role: string) => OutputView; mainLive: boolean }) {
  const [all, setAll] = useState<OutputConfig[]>([]);
  const [role, setRole] = useState("main");
  const [draft, setDraft] = useState<OutputConfig | null>(null);
  const [tab, setTab] = useState<Tab>("general");
  const [displays, setDisplays] = useState<Display[]>([]);
  const [themes, setThemes] = useState<ThemeRow[]>([]);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);

  const load = () => api.outputConfigs().then((c) => (setAll(c), setDraft(structuredClone(c.find((x) => x.role === role) ?? c[0]))));
  useEffect(() => {
    void load();
    api.displays().then(setDisplays).catch(() => {});
    api.listThemes().then(setThemes).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    const c = all.find((x) => x.role === role);
    if (c) setDraft(structuredClone(c));
  }, [role, all]);
  useEffect(() => setMsg(null), [role]);

  if (!draft) return <p className="wp-note">Loading outputs…</p>;
  const set = (p: Partial<OutputConfig>) => setDraft({ ...draft, ...p });
  const saved = all.find((x) => x.role === role);
  const dirty = JSON.stringify(saved) !== JSON.stringify(draft);
  const disp = draft.display_index != null ? displays.find((d) => d.index === draft.display_index) : undefined;
  const W = draft.custom?.w ?? disp?.width ?? 1920;
  const H = draft.custom?.h ?? disp?.height ?? 1080;
  const m = draft.margins;

  const save = async () => {
    try {
      await api.saveOutputConfig(draft);
      setMsg({ text: views(role).condition === "none" ? "Saved. Used the next time this output opens." : "Saved and applied to the open output." });
      await load();
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
  };

  return (
    <div className="oo">
      <aside className="oo-list">
        {all.map((o) => (
          <button key={o.role} className={`oo-item ${o.role === role ? "is-selected" : ""}`} onClick={() => (!dirty || confirm("Leave without saving these changes?")) && setRole(o.role)}>
            <span>{o.role === "alternate" ? "Alternate (lower third)" : o.role === "foldback" ? "Foldback (stage)" : "Main output"}</span>
            <small>{outputLabel(views(o.role), undefined)}</small>
          </button>
        ))}
        <div className="oo-sketch" aria-label="How the margins sit on this output" style={{ aspectRatio: `${W} / ${H}` }}>
          <div className="oo-sketch__safe" style={{ left: `${(m.left / W) * 100}%`, right: `${(m.right / W) * 100}%`, top: `${(m.top / H) * 100}%`, bottom: `${(m.bottom / H) * 100}%` }}>
            verse 1<br />
            song text line one
          </div>
          {draft.alpha && <span className="oo-sketch__alpha">alpha</span>}
        </div>
      </aside>
      <div className="oo-main">
        <div className="wp-tabs oo-tabs" role="tablist">
          {(
            [
              ["general", "General"],
              ["themes", "Themes"],
              ["modes", "Logo and Blank"],
            ] as [Tab, string][]
          ).map(([t, l]) => (
            <button key={t} role="tab" aria-selected={tab === t} onClick={() => setTab(t)}>
              {l}
            </button>
          ))}
        </div>

        {tab === "general" && (
          <>
            <fieldset className="oo-set">
              <legend>Output monitor</legend>
              <select
                className="wp-input"
                value={draft.display_index ?? ""}
                onChange={(e) => {
                  const i = e.target.value === "" ? null : Number(e.target.value);
                  set({ display_index: i, display_name: i == null ? null : displays.find((d) => d.index === i)?.name ?? null });
                }}
              >
                <option value="">Window on this screen (for testing)</option>
                {displays.map((d) => (
                  <option key={d.index} value={d.index}>
                    Monitor {d.index + 1}: {d.name} ({d.width}×{d.height}){d.primary ? ", this screen" : ""}
                  </option>
                ))}
              </select>
              {draft.display_name && !displays.some((d) => d.name === draft.display_name) && <p className="oo-warn">“{draft.display_name}” is not connected now. This output stays closed until it is.</p>}
            </fieldset>
            <fieldset className="oo-set">
              <legend>Output position</legend>
              <label className="wp-check">
                <input type="radio" checked={!draft.custom} onChange={() => set({ custom: null })} /> Fill the whole monitor
              </label>
              <label className="wp-check">
                <input type="radio" checked={!!draft.custom} onChange={() => set({ custom: { x: 0, y: 0, w: disp?.width ?? 1920, h: disp?.height ?? 1080 } })} /> Custom position and size (pixels from the monitor's top-left)
              </label>
              {draft.custom && (
                <div className="oo-grid">
                  <Num label="Left" value={draft.custom.x} onChange={(x) => set({ custom: { ...draft.custom!, x } })} />
                  <Num label="Top" value={draft.custom.y} onChange={(y) => set({ custom: { ...draft.custom!, y } })} />
                  <Num label="Width" value={draft.custom.w} onChange={(w) => set({ custom: { ...draft.custom!, w } })} />
                  <Num label="Height" value={draft.custom.h} onChange={(h) => set({ custom: { ...draft.custom!, h } })} />
                </div>
              )}
            </fieldset>
            <fieldset className="oo-set">
              <legend>Screen margins (pixels)</legend>
              <p className="wp-note wp-note--inline">Keeps words away from edges the projector cuts off, or clear of a pulpit or a TV frame.</p>
              <div className="oo-grid">
                <Num label="Left" value={m.left} onChange={(left) => set({ margins: { ...m, left: Math.max(0, left) } })} />
                <Num label="Right" value={m.right} onChange={(right) => set({ margins: { ...m, right: Math.max(0, right) } })} />
                <Num label="Top" value={m.top} onChange={(top) => set({ margins: { ...m, top: Math.max(0, top) } })} />
                <Num label="Bottom" value={m.bottom} onChange={(bottom) => set({ margins: { ...m, bottom: Math.max(0, bottom) } })} />
              </div>
            </fieldset>
            <fieldset className="oo-set">
              <legend>Alpha channel</legend>
              <select className="wp-input" value={draft.alpha ? "on" : "off"} onChange={(e) => set({ alpha: e.target.value === "on" })}>
                <option value="off">Disabled: black behind the words</option>
                <option value="on">Enabled: see-through behind the words, for keying</option>
              </select>
            </fieldset>
            <label className="wp-check">
              <input type="checkbox" checked={draft.show_on_startup} onChange={(e) => set({ show_on_startup: e.target.checked })} /> Show this output when Worship Presenter starts (only on its own monitor)
            </label>
          </>
        )}

        {tab === "themes" && (
          <fieldset className="oo-set">
            <legend>Themes on this output</legend>
            <p className="wp-note wp-note--inline">An item's own Look still wins; otherwise this output can use a different theme from the others, for example plainer words on the stream.</p>
            {(
              [
                ["song", "Songs", "song_theme_id"],
                ["scripture", "Scripture", "scripture_theme_id"],
              ] as const
            ).map(([kind, label, key]) => (
              <label key={kind} className="oo-row">
                <span>{label}</span>
                <select className="wp-input" value={draft[key] ?? ""} onChange={(e) => set({ [key]: e.target.value ? Number(e.target.value) : null } as Partial<OutputConfig>)}>
                  <option value="">Same as the service (theme in use)</option>
                  {themes
                    .filter((t) => t.kind === kind)
                    .map((t) => (
                      <option key={t.id} value={t.id}>
                        {t.name}
                      </option>
                    ))}
                </select>
              </label>
            ))}
          </fieldset>
        )}

        {tab === "modes" && (
          <fieldset className="oo-set">
            <legend>When the operator presses</legend>
            <label className="oo-row">
              <span>Logo</span>
              <select className="wp-input" value={draft.on_logo} onChange={(e) => set({ on_logo: e.target.value as OutputConfig["on_logo"] })}>
                <option value="logo">Show the logo</option>
                <option value="blank">Go black</option>
                <option value="transparent">Go see-through</option>
                <option value="ignore">Keep showing the slide</option>
              </select>
            </label>
            <label className="oo-row">
              <span>Blank</span>
              <select className="wp-input" value={draft.on_blank} onChange={(e) => set({ on_blank: e.target.value as OutputConfig["on_blank"] })}>
                <option value="blank">Go black</option>
                <option value="transparent">Go see-through</option>
                <option value="ignore">Keep showing the slide</option>
              </select>
            </label>
          </fieldset>
        )}

        {msg && <p className={`wp-banner ${msg.bad ? "wp-banner--bad" : ""}`}>{msg.text}</p>}
        <div className="oo-actions">
          <button className="wp-btn" onClick={() => api.openOutput(role, draft.display_index)}>
            Open
          </button>
          <button
            className="wp-btn wp-btn--quiet"
            onClick={() => {
              if (role === "main" && mainLive && !confirm("Main is showing the service. Identify puts a test pattern on the screen the congregation can see for 4 seconds. Continue?")) return;
              void api.identifyOutput(role);
            }}
          >
            Identify
          </button>
          <button className="wp-btn wp-btn--quiet" onClick={() => api.closeOutput(role)}>
            Close
          </button>
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--quiet" disabled={!dirty} onClick={() => saved && setDraft(structuredClone(saved))}>
            Cancel
          </button>
          <button className="wp-btn wp-btn--primary" disabled={!dirty} onClick={save}>
            Save
          </button>
        </div>
        <p className="wp-note">{ROLE_NAMES[role]}: a lost display is never moved onto your screen; one that comes back stays black until you resume it.</p>
      </div>
    </div>
  );
}
