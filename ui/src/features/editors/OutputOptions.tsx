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
  // how the lower third goes over NDI: the real output page (as FreeShow) or the built-in drawing
  const [lowerNdi, setLowerNdi] = useState<"page" | "builtin">("page");
  useEffect(() => {
    api.getSettings().then((x) => setLowerNdi(x.ndi_lower_third === "builtin" ? "builtin" : "page")).catch(() => {});
  }, []);
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
                aria-label="Output monitor"
                value={draft.ndi ? "ndi" : draft.display_index ?? ""}
                onChange={(e) => {
                  // EasyWorship's "NDI Stream": no monitor, the output goes out over NDI
                  if (e.target.value === "ndi") return set({ ndi: draft.ndi ?? { width: 1920, height: 1080, fps: 30 } });
                  const i = e.target.value === "" ? null : Number(e.target.value);
                  set({ ndi: null, display_index: i, display_name: i == null ? null : displays.find((d) => d.index === i)?.name ?? null });
                }}
              >
                <option value="">Window on this screen (for testing)</option>
                {displays.map((d) => (
                  <option key={d.index} value={d.index}>
                    Monitor {d.index + 1}: {d.name} ({d.width}×{d.height}){d.primary ? ", this screen" : ""}
                  </option>
                ))}
                <option value="ndi">NDI Stream</option>
              </select>
              {draft.ndi && (
                <>
                  <label className="oo-row">
                    <span>Output format</span>
                    <select
                      className="wp-input"
                      aria-label="NDI output format"
                      value={`${draft.ndi.width}x${draft.ndi.height}@${draft.ndi.fps}`}
                      onChange={(e) => {
                        const [wh, fps] = e.target.value.split("@");
                        const [width, height] = wh.split("x").map(Number);
                        set({ ndi: { width, height, fps: Number(fps) } });
                      }}
                    >
                      {[
                        [1920, 1080],
                        [1280, 720],
                        [1024, 768],
                        [3840, 2160],
                      ].flatMap(([w, h]) =>
                        [25, 30, 50, 60].map((f) => (
                          <option key={`${w}x${h}@${f}`} value={`${w}x${h}@${f}`}>
                            {w === 3840 ? "4K" : h === 1080 ? "HD" : h === 720 ? "HD" : "XGA"} {w} × {h} {f} Hz
                          </option>
                        )),
                      )}
                    </select>
                  </label>
                  {draft.role === "alternate" && (
                    <label className="oo-row">
                      <span>Lower third over NDI</span>
                      <select
                        className="wp-input"
                        aria-label="Lower third over NDI"
                        value={lowerNdi}
                        onChange={(e) => {
                          const v = e.target.value as "page" | "builtin";
                          setLowerNdi(v);
                          void api.setSetting("ndi_lower_third", v);
                        }}
                      >
                        <option value="page">The output page: moving video, animations, everything (as FreeShow)</option>
                        <option value="builtin">Built-in drawing: exact see-through edges, still pictures only</option>
                      </select>
                    </label>
                  )}
                  <p className="oo-note">
                    Sent over NDI as “{draft.role === "main" ? "Worship Presenter Main" : draft.role === "alternate" ? "Worship Presenter Alternate" : "Worship Presenter Foldback"}”, with no window
                    on any monitor{draft.role === "alternate" ? " (the lower-third look, see-through for keying)" : ""}. As in FreeShow, the real output page is sent: moving video, text
                    animations and transitions exactly as on the screens. In OBS add an NDI source (DistroAV plugin); vMix lists it under NDI. Needs the NDI Runtime (Settings → Outputs → NDI).
                    {draft.role === "alternate" ? " A change of the lower-third choice applies when the NDI output is next started (Apply)." : ""}
                  </p>
                </>
              )}
              {!draft.ndi && draft.display_name && !displays.some((d) => d.name === draft.display_name) && <p className="oo-warn">“{draft.display_name}” is not connected now. This output stays closed until it is.</p>}
              <div className="oo-actions">
                <button
                  className="wp-btn wp-btn--quiet"
                  onClick={async () => {
                    if (mainLive && !confirm("Something is on the screens. Identify shows a big number on every monitor, the projector too, for 4 seconds. Continue?")) return;
                    try {
                      const n = await api.identifyMonitors();
                      setMsg({ text: `Showing a number on ${n} monitor${n === 1 ? "" : "s"} for 4 seconds. Choose the matching “Monitor” above.` });
                      api.displays().then(setDisplays).catch(() => {});
                    } catch (e) {
                      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
                    }
                  }}
                >
                  Identify screens
                </button>
                <button className="wp-btn wp-btn--quiet" onClick={() => api.displays().then(setDisplays).catch(() => {})} title="Look again for monitors, after plugging one in">
                  Refresh list
                </button>
              </div>
            </fieldset>
            <fieldset className="oo-set" disabled={!!draft.ndi}>
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
            <fieldset className="oo-set" disabled={!!draft.ndi}>
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
            disabled={views(role).condition === "none" || views(role).condition === "disconnected"}
            title={views(role).condition === "none" ? "Open this output first; or use Identify screens to number every monitor" : "Test pattern on this output for 4 seconds"}
            onClick={() => {
              if (role === "main" && mainLive && !confirm("Main is showing the service. Identify puts a test pattern on the screen the congregation can see for 4 seconds. Continue?")) return;
              void api.identifyOutput(role);
            }}
          >
            Test pattern
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
        <p className="wp-note">{ROLE_NAMES[role]}: a lost display is never moved onto your screen; one that comes back stays black until you choose Resume or Go live.</p>
      </div>
    </div>
  );
}
