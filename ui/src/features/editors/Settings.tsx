import { StageLayoutEditor } from "../../StageLayoutEditor";
import { useEffect, useState } from "react";
import { api, pickCsvSave, pickFolder, pickRecording, pickTextSave, type SpeechStatus, type NdiStatus, type RemoteStatus, type ServiceInterval, type UsageRow } from "../../api";
import { OutputOptions } from "./OutputOptions";
import { printUsage } from "../../print";
import { BRAND_DEFAULT, BRAND_NAMES, parseBrand, type Brand } from "../../theme/brand";
import { FontPicker } from "../../theme/FontPicker";
import { ACTION_NAMES, DEFAULT_KEYS, RESERVED, conflict, isSingleLetter, keyName, usePrefs, type Action } from "../../ui/prefs";
import type { OutputView } from "../../live/outputs";

// Settings (design brief §5): compact category list, one 680 px column of labelled
// rows with short explanations.

type Cat = "appearance" | "brand" | "import" | "backups" | "profiles" | "sync" | "scripture" | "stage" | "buttons" | "keyboard" | "projection" | "outputs" | "intervals" | "speech" | "remote" | "library" | "about";
const CATS: { id: Cat; label: string }[] = [
  { id: "appearance", label: "Appearance" },
  { id: "brand", label: "Brand kit" },
  { id: "import", label: "Import" },
  { id: "backups", label: "Backups" },
  { id: "profiles", label: "Profiles" },
  { id: "sync", label: "Sync" },
  { id: "scripture", label: "Scripture display" },
  { id: "stage", label: "Stage display" },
  { id: "keyboard", label: "Keyboard" },
  { id: "buttons", label: "Buttons and MIDI" },
  { id: "projection", label: "Projection" },
  { id: "outputs", label: "Outputs" },
  { id: "intervals", label: "Service intervals" },
  { id: "speech", label: "Scripture recognition" },
  { id: "remote", label: "Phone remote" },
  { id: "library", label: "Library" },
  { id: "about", label: "About" },
];

function Row({ name, hint, children }: { name: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="wp-setting wp-setting--two">
      <div>
        <div className="wp-setting__name">{name}</div>
        {hint && <div className="wp-setting__hint">{hint}</div>}
      </div>
      <div className="wp-row-inline">{children}</div>
    </div>
  );
}

export function Settings({
  uiTheme,
  setUiTheme,
  views,
  mainLive,
  initial,
  onOpen,
  onResetPanes,
}: {
  uiTheme: "dark" | "light" | "system";
  setUiTheme: (t: "dark" | "light" | "system") => void;
  views: (role: string) => OutputView;
  mainLive: boolean;
  onOpenOutputs?: () => void;
  /** Open at this page (the Help menu's About and Keyboard shortcuts). */
  initial?: string;
  /** Opens the import, backup or profile windows. */
  onOpen?: (what: "import" | "backups" | "profiles") => void;
  /** Puts the service window's panes back to their first sizes. */
  onResetPanes?: () => void;
}) {
  const [cat, setCat] = useState<Cat>(() => (initial as Cat) || "appearance");
  useEffect(() => {
    if (initial) setCat(initial as Cat);
  }, [initial]);
  const [depth, setDepth] = useState(() => localStorage.getItem("wp-depth") || "3d");
  const [prefs, setPrefs] = usePrefs();
  const [core, setCore] = useState<Record<string, string>>({});
  const [capturing, setCapturing] = useState<Action | null>(null);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  const [folder, setFolder] = useState("");

  useEffect(() => {
    api.getSettings().then(setCore).catch(() => {});
    api.backupsFolder().then((f) => setFolder(f.replace(/[\\/]backups$/, ""))).catch(() => {});
  }, []);

  const setCoreKey = async (key: string, value: string) => {
    try {
      await api.setSetting(key, value);
      setCore((c) => ({ ...c, [key]: value }));
      if (key === "lines_per_slide") await api.setLayout(Number(value)).catch(() => {});
      setMsg(null);
    } catch (e) {
      setMsg({ text: String(e), bad: true });
    }
  };

  // key capture for rebinding: one keydown, checked for conflicts before saving
  useEffect(() => {
    if (!capturing) return;
    const f = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" && capturing !== "blank") return setCapturing(null);
      if (e.ctrlKey || e.altKey || e.metaKey) return setMsg({ text: "Use a key without Ctrl or Alt; those are kept for commands.", bad: true });
      if (RESERVED.includes(e.key)) return setMsg({ text: `${keyName(e.key)} is kept by Worship Presenter (Go live, regions, sections).`, bad: true });
      const other = conflict(prefs, e.key, capturing);
      if (other) return setMsg({ text: `${keyName(e.key)} is already used for ${ACTION_NAMES[other]}. Remove it there first.`, bad: true });
      setPrefs({ ...prefs, keys: { ...prefs.keys, [capturing]: [...prefs.keys[capturing], e.key] } });
      setMsg({ text: `${ACTION_NAMES[capturing]}: added ${keyName(e.key)}.` });
      setCapturing(null);
    };
    window.addEventListener("keydown", f, true);
    return () => window.removeEventListener("keydown", f, true);
  }, [capturing, prefs, setPrefs]);


  return (
    <div className="wp-page wp-settings">
      <nav className="wp-settings__nav" aria-label="Settings">
        <h1 className="wp-editor-title">Settings</h1>
        {CATS.map((c) => (
          <button key={c.id} className={`wp-settings__cat ${cat === c.id ? "is-selected" : ""}`} aria-current={cat === c.id} onClick={() => setCat(c.id)}>
            {c.label}
          </button>
        ))}
      </nav>
      <main className="wp-settings__main">
        <div className="wp-settings__col">
          {msg && <p className={`wp-banner ${msg.bad ? "wp-banner--bad" : ""}`}>{msg.text}</p>}

          {cat === "appearance" && (
            <>
              <h2>Appearance</h2>
              <Row name="Pane sizes" hint="Puts Schedule, Preview, Live and the library back to their first sizes">
                <button className="wp-btn" onClick={() => onResetPanes?.()}>
                  Reset pane sizes
                </button>
              </Row>
              <Row name="Theme" hint="Colours of this window only. The screens follow their own themes.">
                <div className="wp-tabs" role="radiogroup" aria-label="Theme">
                  {(["dark", "light", "system"] as const).map((t) => (
                    <button key={t} role="radio" aria-checked={uiTheme === t} aria-selected={uiTheme === t} onClick={() => setUiTheme(t)}>
                      {t === "dark" ? "Dark" : t === "light" ? "Light" : "Follow Windows"}
                    </button>
                  ))}
                </div>
              </Row>
              <Row name="Look" hint="3D gives buttons, cards and screens depth. Flat is plainer and a little lighter on old laptops.">
                <div className="wp-tabs" role="radiogroup" aria-label="Look">
                  {(["3d", "flat"] as const).map((v) => (
                    <button
                      key={v}
                      role="radio"
                      aria-checked={depth === v}
                      aria-selected={depth === v}
                      onClick={() => (localStorage.setItem("wp-depth", v), (document.documentElement.dataset.depth = v), setDepth(v))}
                    >
                      {v === "3d" ? "3D" : "Flat"}
                    </button>
                  ))}
                </div>
              </Row>
              <Row name="Low-spec interface" hint="Cuts hover and menu animations in this window. For older laptops.">
                <input type="checkbox" checked={prefs.lowSpecUi} onChange={(e) => setPrefs({ ...prefs, lowSpecUi: e.target.checked })} aria-label="Low-spec interface" />
              </Row>
            </>
          )}

          {cat === "keyboard" && (
            <>
              <h2>Keyboard</h2>
              <p className="wp-note">These keys change the screens only when the Live pane is focused (it is focused after Go live). Typing anywhere else never changes the screens.</p>
              <Row name="Single-letter shortcuts" hint="L for logo and C for clear. Turn off if letters are pressed by accident.">
                <input type="checkbox" checked={prefs.singleLetter} onChange={(e) => setPrefs({ ...prefs, singleLetter: e.target.checked })} aria-label="Single-letter shortcuts" />
              </Row>
              {(Object.keys(ACTION_NAMES) as Action[]).map((a) => (
                <Row key={a} name={ACTION_NAMES[a]}>
                  {prefs.keys[a].map((k) => (
                    <span key={k} className={`wp-key ${!prefs.singleLetter && isSingleLetter(k) ? "is-off" : ""}`}>
                      {keyName(k)}
                      <button className="wp-key__x" aria-label={`Remove ${keyName(k)}`} onClick={() => setPrefs({ ...prefs, keys: { ...prefs.keys, [a]: prefs.keys[a].filter((x) => x !== k) } })}>
                        ×
                      </button>
                    </span>
                  ))}
                  <button className="wp-btn wp-btn--quiet" onClick={() => (setMsg({ text: `Press a key for ${ACTION_NAMES[a]}.` }), setCapturing(a))}>
                    {capturing === a ? "Press a key…" : "Add key"}
                  </button>
                </Row>
              ))}
              <div className="wp-actions wp-actions--bare">
                <button className="wp-btn wp-btn--quiet" onClick={() => (setPrefs({ ...prefs, keys: DEFAULT_KEYS }), setMsg({ text: "Keys reset." }))}>
                  Reset keys
                </button>
              </div>
              <p className="wp-note">Always: F5 Go live (not while typing), Ctrl+K commands, Ctrl+F song search, Ctrl+G quick scripture, F6 next region, 1–9 sections on the Live pane, Ctrl+Z back to the previous live item.</p>
            </>
          )}

          {cat === "projection" && (
            <>
              <h2>Projection</h2>
              <Row name="Lines per slide" hint="Default for songs; each song or the Live pane can change it.">
                <select className="wp-input" value={core.lines_per_slide ?? "2"} onChange={(e) => void setCoreKey("lines_per_slide", e.target.value)}>
                  {[1, 2, 3, 4, 5, 6].map((n) => (
                    <option key={n}>{n}</option>
                  ))}
                </select>
              </Row>
              <Row name="Show the last slide after a restart" hint="Off: after a power cut the slide returns to Preview and the screens stay as they are until you resume.">
                <input type="checkbox" checked={core.project_on_restart === "true"} onChange={(e) => void setCoreKey("project_on_restart", String(e.target.checked))} />
              </Row>
              <Row name="Advance the schedule after Go live" hint="When an item from the schedule goes live, the next item comes into Preview, ready for the next Go live.">
                <input type="checkbox" checked={core.advance_after_go_live === "true"} onChange={(e) => void setCoreKey("advance_after_go_live", String(e.target.checked))} />
              </Row>
              <Row name="Font fallback list" hint="Fonts tried, in order, for any letter a theme's font does not have. Keep Noto Sans in the list: it has ɛ, ɔ, ŋ and the tone marks.">
                <input
                  className="wp-input wp-input--wide"
                  defaultValue={core.font_fallback ?? "Noto Sans, Segoe UI"}
                  onBlur={(e) => void setCoreKey("font_fallback", e.target.value)}
                  aria-label="Font fallback list, separated by commas"
                />
              </Row>
              <Row name="Low-spec projection" hint="Slides change with a cut instead of animations. Use on weak computers or when video stutters.">
                <input type="checkbox" checked={core.low_spec_mode === "true"} onChange={(e) => void setCoreKey("low_spec_mode", String(e.target.checked))} />
              </Row>
            </>
          )}

          {cat === "outputs" && (
            <>
              <h2>Outputs</h2>
              <div className="wp-settings__wide">
                <OutputOptions views={views} mainLive={mainLive} />
              </div>
              <NdiPanel />
            </>
          )}

          {cat === "intervals" && <IntervalsPanel />}

          {cat === "speech" && <SpeechPanel core={core} setCoreKey={setCoreKey} />}

          {cat === "remote" && <RemotePanel />}

          {cat === "brand" && <BrandKit />}
          {cat === "buttons" && <ButtonsPanel />}
          {cat === "sync" && <SyncPanel />}
          {cat === "stage" && <StageLayoutEditor />}
          {cat === "scripture" && (
            <section>
              <h2>Scripture display</h2>
              <p className="wp-note">As EasyWorship's Scripture options. They apply the next time a reading goes to Preview or Live.</p>
              <Row name="Verse numbers" hint="Before each verse's words">
                <select className="wp-input" value={core.scripture_verse_numbers ?? ""} onChange={(e) => void setCoreKey("scripture_verse_numbers", e.target.value)} aria-label="Verse numbers">
                  <option value="">Hide</option>
                  <option value="on">Show (16 For God…)</option>
                  <option value="super">Show small and raised (¹⁶ For God…)</option>
                </select>
              </Row>
              <Row name="Verses per slide" hint="EasyWorship's “break on new verse”: one verse per slide, or short verses flowing together">
                <select className="wp-input" value={core.scripture_verses_per_slide || "1"} onChange={(e) => void setCoreKey("scripture_verses_per_slide", e.target.value)} aria-label="Verses per slide">
                  {["1", "2", "3", "4"].map((n) => (
                    <option key={n} value={n}>
                      {n === "1" ? "One verse per slide" : `Up to ${n} verses per slide`}
                    </option>
                  ))}
                </select>
              </Row>
              <Row name="Reference location" hint="Where the reference layer of the theme shows">
                <select className="wp-input" value={core.scripture_ref_where ?? ""} onChange={(e) => void setCoreKey("scripture_ref_where", e.target.value)} aria-label="Reference location">
                  <option value="">Every slide</option>
                  <option value="first">First slide only</option>
                  <option value="last">Last slide only</option>
                  <option value="none">Never</option>
                </select>
              </Row>
              <Row name="Reference shows" hint="The whole reading, or the verses on that slide">
                <select className="wp-input" value={core.scripture_ref_scope ?? ""} onChange={(e) => void setCoreKey("scripture_ref_scope", e.target.value)} aria-label="Reference shows">
                  <option value="">The whole passage (John 3:16-18)</option>
                  <option value="verse">This slide's verses (John 3:17)</option>
                </select>
              </Row>
              <Row name="Abbreviate book names" hint="Jn 3:16 instead of John 3:16 (EasyWorship's abbreviations)">
                <input type="checkbox" checked={core.scripture_abbrev === "true"} onChange={(e) => void setCoreKey("scripture_abbrev", e.target.checked ? "true" : "false")} aria-label="Abbreviate book names" />
              </Row>
              <Row name="Show reference only" hint="Hide the scripture text (e.g. a lower third naming the reading)">
                <input type="checkbox" checked={core.scripture_ref_only === "true"} onChange={(e) => void setCoreKey("scripture_ref_only", e.target.checked ? "true" : "false")} aria-label="Show reference only" />
              </Row>
            </section>
          )}
          {cat === "import" && (
            <section>
              <h2>Import</h2>
              <p className="wp-note">Bring songs, Bibles and presentations into this profile. A backup is made first, and nothing is added until you have checked the list.</p>
              <Row name="Songs and Bibles" hint="EasyWorship (songs and .ewb Bibles), Bible Show, OpenLyrics, OpenSong, ProPresenter, SongSelect, ChordPro, plain text">
                <button className="wp-btn" onClick={() => onOpen?.("import")}>
                  Import songs or Bibles…
                </button>
              </Row>
              <Row name="Presentations" hint="PowerPoint or PDF: Library → Presentations → right-click → Import">
                <span className="wp-note wp-note--inline">In the library</span>
              </Row>
              <Row name="Themes" hint="Worship Presenter, EasyWorship (.ewtx) or ProPresenter (.proTheme): Library → Themes → right-click → Import Theme File">
                <span className="wp-note wp-note--inline">In the library</span>
              </Row>
            </section>
          )}
          {cat === "backups" && (
            <section>
              <h2>Backups</h2>
              <p className="wp-note">A backup is made every day you use Worship Presenter and before every import, restore or deletion. Restoring makes a backup of the current state first, so it can be undone.</p>
              <Row name="Backups" hint="See, make, restore and export backups">
                <button className="wp-btn" onClick={() => onOpen?.("backups")}>
                  Open backups…
                </button>
              </Row>
            </section>
          )}
          {cat === "profiles" && (
            <section>
              <h2>Profiles</h2>
              <p className="wp-note">Each profile has its own songs, Bibles, media, themes and settings (for example a main church and a youth service).</p>
              <Row name="Profiles" hint="Switch, add, rename or delete profiles">
                <button className="wp-btn" onClick={() => onOpen?.("profiles")}>
                  Open profiles…
                </button>
              </Row>
            </section>
          )}
          {cat === "library" && (
            <>
              <h2>Library</h2>
              <Row name="Main hymnbook" hint="A number typed alone in the quick box (“555”) opens this hymnbook's song. Other hymnbooks: “NCAN 12”.">
                <HymnalPick value={core.quick_hymnal ?? ""} onChange={(v) => void setCoreKey("quick_hymnal", v)} />
              </Row>
              <Row name="Rebuild search keys" hint="If a song you know is there does not come up when searching, this rewrites every song's search entry. Takes a few seconds.">
                <button className="wp-btn" onClick={async () => { try { const n = await api.rebuildSearch(); setMsg({ text: `Search keys rebuilt for ${n.toLocaleString()} songs.` }); } catch (e) { setMsg({ text: String(e), bad: true }); } }}>
                  Rebuild
                </button>
              </Row>
              <Row name="Reset songs to the theme in use" hint="Removes look and lines-per-slide settings saved on individual songs, so every song follows the theme in use.">
                <button className="wp-btn" onClick={async () => { if (!confirm("Reset every song to the theme in use?")) return; try { const n = await api.resetSongLooks(); setMsg({ text: n ? `${n} songs reset.` : "Every song already follows the theme in use." }); } catch (e) { setMsg({ text: String(e), bad: true }); } }}>
                  Reset
                </button>
              </Row>
            </>
          )}

          {cat === "about" && (
            <>
              <div className="wp-brand">
                <img className="wp-brand__dark" src="/brand/wordmark-dark.png" alt="Worship Presenter" />
                <img className="wp-brand__light" src="/brand/wordmark-light.png" alt="Worship Presenter" />
              </div>
              <h2>About</h2>
              <Row name="Updates" hint="New versions come from the church's GitHub releases; only installers signed with the church's key are accepted.">
                <button
                  className="wp-btn"
                  onClick={async () => {
                    try {
                      const u = await api.updateCheck();
                      if (!u) return setMsg({ text: "Worship Presenter is up to date." });
                      if (confirm(`Worship Presenter ${u.version} is available.${u.notes ? "\n\n" + u.notes : ""}\n\nInstall it and restart now?`)) await api.updateInstall();
                    } catch (e) {
                      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
                    }
                  }}
                >
                  Check for updates
                </button>
              </Row>
              <Row name="When something goes wrong" hint="The app keeps a log of crashes and errors (no song words or notes). Send the report with a description of what happened.">
                <button className="wp-btn" onClick={() => void api.openLogs().catch((e) => setMsg({ text: String(e), bad: true }))}>
                  Open the logs folder
                </button>
                <button
                  className="wp-btn"
                  onClick={async () => {
                    const r = await api.diagnosticReport();
                    try {
                      await navigator.clipboard.writeText(r);
                      setMsg({ text: "The report is copied: paste it into your message." });
                    } catch {
                      setMsg({ text: r });
                    }
                  }}
                >
                  Copy a diagnostic report
                </button>
              </Row>
              <Row name="Worship Presenter" hint="Version 0.1.0">
                <span />
              </Row>
              {folder && (
                <Row name="This profile's folder" hint={folder}>
                  <span />
                </Row>
              )}
            </>
          )}
        </div>
      </main>
    </div>
  );
}

function NdiPanel() {
  const [st, setSt] = useState<NdiStatus | null>(null);
  const [name, setName] = useState("Worship Presenter Lower Third");
  const [fps, setFps] = useState(30);
  const [groups, setGroups] = useState("");
  const [drawing, setDrawing] = useState<"builtin" | "capture">("builtin");
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  useEffect(() => {
    const load = () => api.ndiStatus().then(setSt).catch(() => {});
    load();
    const t = setInterval(load, 1000);
    return () => clearInterval(t);
  }, []);
  const go = async (f: () => Promise<unknown>, ok: string) => {
    try {
      await f();
      setMsg({ text: ok });
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
  };
  const running = (s: string) => st?.sources.find((x) => x.source === s);
  const results = () =>
    [
      `Worship Presenter NDI prototype results (${new Date().toLocaleString()})`,
      `NDI Runtime: ${st?.runtime ?? st?.problem ?? "unknown"}`,
      ...(st?.sources ?? []).map(
        (x) =>
          `${x.source} "${x.name}": ${x.width}x${x.height}, sent ${x.sent} frames in ${x.seconds} s (${x.seconds ? (x.sent / x.seconds).toFixed(1) : 0} fps, target ${x.fps}), captured ${x.captured} (${x.seconds ? (x.captured / x.seconds).toFixed(1) : 0} per s), last frame processing ${x.process_ms.toFixed(1)} ms, receivers ${x.connections}`,
      ),
      "Please add: the computer and graphics card, whether the edges look clean in OBS/vMix, Task Manager CPU for Worship Presenter, and any delay you notice.",
    ].join("\n");
  return (
    <section className="ndi">
      <h2>NDI</h2>
      <p className="wp-note">
        {st?.runtime ? `NDI Runtime ${st.runtime}.` : st?.problem ?? "Checking for the NDI Runtime…"} NDI® is a registered trademark of Vizrt NDI AB.
      </p>
      {!st?.runtime && (
        <Row name="NDI Runtime" hint="Free from NDI. Install it, then restart Worship Presenter.">
          <button className="wp-btn" onClick={() => go(api.openNdiRuntimePage, "The NDI download page is open in your browser.")}>
            Get the NDI Runtime
          </button>
        </Row>
      )}
      <Row name="NDI groups" hint="Optional, e.g. Church. Receivers must be in the same group to see the sources. Empty: everyone on the network.">
        <input className="wp-input" value={groups} onChange={(e) => setGroups(e.target.value)} placeholder="Public" aria-label="NDI groups" />
      </Row>
      <Row name="Test pattern" hint="Moving colour bars with a see-through half for 30 seconds. Checks OBS or vMix receives NDI, before anything else.">
        <button className="wp-btn" disabled={!st?.runtime || !!running("test")} onClick={() => go(() => api.ndiTestPattern("Worship Presenter Test", 30), "Sending “Worship Presenter Test” for 30 seconds. Add it as an NDI source in OBS or vMix.")}>
          {running("test") ? "Sending…" : "Send test pattern"}
        </button>
      </Row>
      <Row name="Lower third over NDI" hint="The lower third with real transparency, for keying in OBS or vMix. No monitor or window needed. Sends 1 frame a second until a receiver connects.">
        <select className="wp-input" value={drawing} onChange={(e) => setDrawing(e.target.value as "builtin" | "capture")} aria-label="How the lower third is drawn">
          <option value="builtin">Built-in (exact transparency)</option>
          <option value="capture">Window capture (fallback)</option>
        </select>
        <input className="wp-input" value={name} onChange={(e) => setName(e.target.value)} aria-label="NDI name" />
        <select className="wp-input" value={fps} onChange={(e) => setFps(Number(e.target.value))} aria-label="Frames per second">
          {[25, 30, 50, 60].map((n) => (
            <option key={n} value={n}>
              {n} fps
            </option>
          ))}
        </select>
        {running("lower_third") ? (
          <button className="wp-btn" onClick={() => go(() => api.ndiStop("lower_third"), "Stopped.")}>
            Stop
          </button>
        ) : (
          <button className="wp-btn" disabled={!st?.runtime || (drawing === "capture" && !st?.capture_supported)} onClick={() => go(() => api.ndiStart("lower_third", name, fps, groups, drawing), `Sending “${name}”${groups.trim() ? ` in the group “${groups.trim()}”` : ""}.`)}>
            Start
          </button>
        )}
      </Row>
      <Row name="Main over NDI" hint="The main output as it looks on the projector (no transparency). Open the main output first.">
        {running("main") ? (
          <button className="wp-btn" onClick={() => go(() => api.ndiStop("main"), "Stopped.")}>
            Stop
          </button>
        ) : (
          <button className="wp-btn" disabled={!st?.runtime || !st?.capture_supported} onClick={() => go(() => api.ndiStart("main", "Worship Presenter Main", 30, groups, "capture"), "Sending “Worship Presenter Main”.")}>
            Start
          </button>
        )}
      </Row>
      {(st?.sources ?? []).filter((x) => x.source !== "test").map((x) => (
        <p key={x.source} className="wp-note wp-num">
          <label className="wp-check" style={{ marginRight: 12 }} title="Sends everything this computer plays (music, video sound) with this NDI source">
            <input type="checkbox" checked={!!x.audio} onChange={(e) => go(() => api.ndiAudio(x.source, e.target.checked), e.target.checked ? `The computer's sound now goes with “${x.name}”.` : "Sound stopped.")} /> With the computer's sound
          </label>
          {x.name}: {x.width}×{x.height}, {x.seconds ? (x.sent / x.seconds).toFixed(1) : "0"} fps sent, {x.seconds ? (x.captured / x.seconds).toFixed(1) : "0"} captures/s, {x.process_ms.toFixed(1)} ms per frame{x.connections ? "" : " (1 frame a second until someone connects)"}, {x.connections} receiver{x.connections === 1 ? "" : "s"}
        </p>
      ))}
      {msg && <p className={`wp-banner ${msg.bad ? "wp-banner--bad" : ""}`}>{msg.text}</p>}
      <div className="wp-actions wp-actions--bare">
        <button className="wp-btn wp-btn--quiet" onClick={() => navigator.clipboard.writeText(results()).then(() => setMsg({ text: "Results copied. Paste them to me." }))}>
          Copy prototype results
        </button>
      </div>
    </section>
  );
}

const ROLE_LABEL = { operator: "Controls the screens", stage: "Stage display", viewer: "Watch only" } as const;
const ago = (s: number) => (s < 60 ? "just now" : s < 3600 ? `${Math.floor(s / 60)} min ago` : `${Math.floor(s / 3600)} h ago`);

function RemotePanel() {
  const [st, setSt] = useState<RemoteStatus | null>(null);
  const [everywhere, setEverywhere] = useState(false);
  const [port, setPort] = useState(7300);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  useEffect(() => {
    const load = () => api.remoteStatus().then(setSt).catch(() => {});
    load();
    const t = setInterval(load, 1500);
    return () => clearInterval(t);
  }, []);
  const go = async (f: () => Promise<unknown>, ok?: string) => {
    try {
      await f();
      setMsg(ok ? { text: ok } : null);
      setSt(await api.remoteStatus());
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
  };
  return (
    <>
      <h2>Phone remote</h2>
      <p className="wp-note">
        Phones and tablets on the church Wi-Fi can drive the screens, or show the stage display, after entering a PIN. It is off until you start it. Every press from a phone goes through the same checks as the desk, and is logged.
      </p>
      {!st?.running ? (
        <>
          <Row name="Listen on" hint={st?.lan_ip ? `This computer's address on the network: ${st.lan_ip}` : "This computer does not seem to be on a network."}>
            <select className="wp-input" value={everywhere ? "all" : "lan"} onChange={(e) => setEverywhere(e.target.value === "all")}>
              <option value="lan">The church network only</option>
              <option value="all">Every network on this computer</option>
            </select>
          </Row>
          <Row name="Port" hint="Change only if another program already uses it.">
            <input className="wp-input wp-input--short" type="number" value={port} min={1024} max={65535} onChange={(e) => setPort(Number(e.target.value))} />
          </Row>
          <div className="wp-actions wp-actions--bare">
            <button className="wp-btn" onClick={() => go(() => api.remoteStart(everywhere, port), "The remote is on. Phones join with the PIN or the QR code.")}>
              Start the remote
            </button>
          </div>
        </>
      ) : (
        <>
          <div className="follow-box">
            <label className="wp-check">
              <input type="checkbox" checked={!!st.follow} onChange={(e) => go(() => api.remoteFollow(e.target.checked), e.target.checked ? "Following along is open: phones on the church Wi-Fi can scan the code." : "Following along is closed.")} />{" "}
              <strong>Follow along on phones</strong>: the congregation sees the words on the screens on their own phones (English, Fante or both, large print). Read-only; no PIN.
            </label>
            {st.follow && st.follow_qr_svg && (
              <div className="remote-join">
                <div className="remote-qr" aria-label={`QR code for ${st.follow_url}`} dangerouslySetInnerHTML={{ __html: st.follow_qr_svg }} />
                <div>
                  <div className="wp-setting__hint">Scan, or open: {st.follow_url}</div>
                  <div className="wp-setting__hint">Phones must be on the same Wi-Fi as this computer.</div>
                  <button className="wp-btn" onClick={() => go(() => followSlide(st.follow_url!, st.follow_qr_svg!), "A slide with the code is in Presentations: put it up before the service.")}>
                    Make a slide with this code
                  </button>
                </div>
              </div>
            )}
          </div>
          <div className="remote-join">
            {st.qr_svg && <div className="remote-qr" aria-label={`QR code for ${st.url}`} dangerouslySetInnerHTML={{ __html: st.qr_svg }} />}
            <div>
              <div className="wp-setting__hint">PIN</div>
              <div className="remote-pin wp-num">{st.pin}</div>
              <div className="wp-setting__hint">Phones: {st.url}</div>
              <div className="wp-setting__hint">Stage tablet: {st.stage_url}</div>
              <div className="wp-actions wp-actions--bare">
                <button className="wp-btn wp-btn--quiet" onClick={() => go(() => api.remoteNewPin(), "New PIN. Phones already joined stay joined.")}>
                  New PIN
                </button>
                <button className="wp-btn wp-btn--quiet" onClick={() => go(() => api.remoteStop(), "The remote is off and every phone was signed out.")}>
                  Stop the remote
                </button>
              </div>
            </div>
          </div>
          <h2>Joined ({st.sessions.length})</h2>
          {st.sessions.map((x) => (
            <Row key={x.id} name={x.device} hint={`${ROLE_LABEL[x.role]} · ${x.ip} · joined ${ago(x.joined_secs_ago)}, last seen ${ago(x.idle_secs)}`}>
              <button className="wp-btn wp-btn--quiet" onClick={() => go(() => api.remoteRevoke(x.id), `${x.device} was signed out.`)}>
                Sign out
              </button>
            </Row>
          ))}
          {st.sessions.length > 0 && (
            <div className="wp-actions wp-actions--bare">
              <button className="wp-btn" onClick={() => go(() => api.remoteEndAll(), "Every phone was signed out. They join again with the PIN.")}>
                End of service: sign everyone out
              </button>
            </div>
          )}
          <h2>Recent presses</h2>
          <ul className="remote-log">
            {st.log.map((l, i) => (
              <li key={i} className={l.ok ? "" : "is-bad"}>
                <span className="wp-num">{new Date(l.at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}</span> {l.device}: {l.action}
              </li>
            ))}
            {!st.log.length && <li className="wp-note">Nothing yet.</li>}
          </ul>
        </>
      )}
      {msg && <p className={`wp-banner ${msg.bad ? "wp-banner--bad" : ""}`}>{msg.text}</p>}
    </>
  );
}

const DAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

function IntervalsPanel() {
  const [list, setList] = useState<ServiceInterval[]>([]);
  const [now, setNow] = useState<string | null>(null);
  const [draft, setDraft] = useState<ServiceInterval>({ id: null, name: "First service", weekday: 0, start: "07:00", end: "09:30" });
  const [from, setFrom] = useState(() => new Date(Date.now() - 30 * 864e5).toISOString().slice(0, 10));
  const [to, setTo] = useState(() => new Date().toISOString().slice(0, 10));
  const [rows, setRows] = useState<UsageRow[] | null>(null);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  const load = () => {
    api.serviceIntervals().then(setList).catch(() => {});
    api.currentServiceInterval().then(setNow).catch(() => {});
  };
  useEffect(load, []);
  const go = async (f: () => Promise<unknown>, ok?: string) => {
    try {
      await f();
      setMsg(ok ? { text: ok } : null);
      load();
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
  };
  return (
    <>
      <h2>Service intervals</h2>
      <p className="wp-note">Your service times. Every song that goes live is recorded with the service it was in, for your CCLI report. {now ? `Now: ${now}.` : "No service is on right now."}</p>
      {list.map((i) => (
        <Row key={i.id} name={i.name} hint={`${i.weekday < 0 ? "Every day" : DAYS[i.weekday]}, ${i.start} to ${i.end}`}>
          <button className="wp-btn wp-btn--quiet" onClick={() => setDraft(i)}>
            Edit
          </button>
          <button className="wp-btn wp-btn--quiet" onClick={() => i.id != null && go(() => api.deleteServiceInterval(i.id!), "Removed.")}>
            Remove
          </button>
        </Row>
      ))}
      <div className="interval-form">
        <input className="wp-input" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} aria-label="Service name" />
        <select className="wp-input" value={draft.weekday} onChange={(e) => setDraft({ ...draft, weekday: Number(e.target.value) })} aria-label="Day">
          <option value={-1}>Every day</option>
          {DAYS.map((d, i) => (
            <option key={d} value={i}>
              {d}
            </option>
          ))}
        </select>
        <input className="wp-input wp-input--short" type="time" value={draft.start} onChange={(e) => setDraft({ ...draft, start: e.target.value })} aria-label="Starts" />
        <span>to</span>
        <input className="wp-input wp-input--short" type="time" value={draft.end} onChange={(e) => setDraft({ ...draft, end: e.target.value })} aria-label="Ends" />
        <button className="wp-btn" onClick={() => go(() => api.saveServiceInterval(draft), draft.id ? "Saved." : "Added.").then(() => setDraft({ id: null, name: "", weekday: 0, start: "10:00", end: "12:00" }))}>
          {draft.id ? "Save" : "Add"}
        </button>
      </div>
      <h2>Song usage report</h2>
      <p className="wp-note">Songs shown between two dates, by service; a song shown several times in one service counts once. Save it for a spreadsheet or CCLI's website.</p>
      <div className="interval-form">
        <input className="wp-input wp-input--short" type="date" value={from} onChange={(e) => setFrom(e.target.value)} aria-label="From" />
        <span>to</span>
        <input className="wp-input wp-input--short" type="date" value={to} onChange={(e) => setTo(e.target.value)} aria-label="To" />
        <button className="wp-btn wp-btn--quiet" onClick={() => go(() => api.usageReport(from, to).then(setRows))}>
          Show
        </button>
        <button
          className="wp-btn"
          onClick={async () => {
            const path = await pickCsvSave(`song-usage-${from}-to-${to}.csv`);
            if (path) await go(async () => setMsg({ text: `Saved ${await api.saveUsageReport(from, to, path)} rows.` }));
          }}
        >
          Save as CSV…
        </button>
        <button
          className="wp-btn"
          disabled={!rows?.length}
          onClick={() => rows && printUsage(`Song usage ${from} to ${to}`, rows.map((r) => ({ title: r.title, author: r.author, ccli: r.ccli, count: r.uses })))}
        >
          Print…
        </button>
      </div>
      {rows && (
        <table className="usage">
          <thead>
            <tr>
              <th>Service</th>
              <th>Song</th>
              <th>Numbers</th>
              <th>CCLI</th>
              <th>Times</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i}>
                <td>{r.interval}</td>
                <td>{r.title}</td>
                <td>{r.numbers}</td>
                <td>{r.ccli}</td>
                <td className="wp-num">{r.uses}</td>
              </tr>
            ))}
            {!rows.length && (
              <tr>
                <td colSpan={5}>No songs were shown in those dates.</td>
              </tr>
            )}
          </tbody>
        </table>
      )}
      {msg && <p className={`wp-banner ${msg.bad ? "wp-banner--bad" : ""}`}>{msg.text}</p>}
    </>
  );
}

function SpeechPanel({ core, setCoreKey }: { core: Record<string, string>; setCoreKey: (k: string, v: string) => Promise<void> | void }) {
  const [test, setTest] = useState<{ transcript: [number, string][]; found: [number, string, string][]; model: string; seconds: number; took_seconds: number } | null>(null);
  const [testProblem, setTestProblem] = useState<string | null>(null);
  const [st, setSt] = useState<SpeechStatus | null>(null);
  const [mics, setMics] = useState<string[]>([]);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  const engine = core.speech_engine || "whisper";
  const refsOnly = core.speech_refs_only !== "false";
  useEffect(() => {
    api.speechMics().then(setMics).catch(() => {});
    const t = setInterval(() => api.speechStatus().then(setSt).catch(() => {}), 600);
    api.speechStatus().then(setSt).catch(() => {});
    return () => clearInterval(t);
  }, []);
  const go = async (f: () => Promise<unknown>, ok?: string) => {
    try {
      await f();
      setMsg(ok ? { text: ok } : null);
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
  };
  const engines: [string, string, string, boolean][] = [
    ["whisper", "Whisper (recommended)", "OpenAI's Whisper, run on this computer: by far the best with accents and a live room. Downloaded once; then works without internet.", true],
    ["deepgram", "Deepgram (online)", "Fast and accurate with accents, streamed live. Needs internet and a Deepgram account (paid per minute of audio); Whisper takes over if the internet drops.", true],
    ["vosk", "Vosk", "Light and offline (about 60 MB). Struggles with accents and noisy rooms.", true],
    ["windows", "Windows Speech", "Built into Windows; nothing to download. Least accurate in a church.", !!st?.windows_speech],
    ["cloud", "A cloud service with your own key", "Not available yet.", false],
  ];
  return (
    <>
      <h2>Scripture recognition (experimental)</h2>
      <p className="wp-note">
        Listens to the preacher's microphone and suggests the Bible references it hears, above the Live pane. It follows the preacher (“verse seventeen”, “the next verse”, “chapter four”, a reference
        spread over two sentences) and, when listening to everything, recognises a verse read aloud without its reference. It never puts anything on the screens: you choose Preview or Go live. English now; Twi and Fante when speech models for them exist. Start and stop it with Listen in the service bar.
      </p>
      <Row name="Speech engine" hint="Which program turns speech into words.">
        <div className="speech-engines" role="radiogroup" aria-label="Speech engine">
          {engines.map(([id, name, hint, ok]) => (
            <label key={id} className={`speech-engine ${ok ? "" : "is-off"}`}>
              <input
                type="radio"
                name="engine"
                disabled={!ok}
                checked={engine === id}
                onChange={async () => {
                  await setCoreKey("speech_engine", id);
                  // already listening: switch at once (same session, microphone and gain)
                  if (st?.listening) {
                    try {
                      await api.speechStart(id, core.speech_mic || null, refsOnly);
                      window.dispatchEvent(new CustomEvent("wp-toast", { detail: `Switched to ${name}.` }));
                    } catch (e) {
                      window.dispatchEvent(new CustomEvent("wp-toast", { detail: String(e instanceof Error ? e.message : e) }));
                    }
                  }
                }}
              />
              <span>
                <strong>{name}</strong>
                <small>{hint}</small>
              </span>
            </label>
          ))}
        </div>
      </Row>
      {engine === "whisper" && (
        <Row
          name="Whisper model"
          hint={
            st?.whisper_models?.length
              ? `Downloaded: ${st.whisper_models.join(", ")}. Small is more accurate; Base is lighter on older computers.`
              : "Needs internet once. Base (about 150 MB) suits most computers; Small (about 490 MB) is more accurate on a recent one."
          }
        >
          <select className="wp-input" value={core.speech_whisper_model || "small.en"} onChange={(e) => void setCoreKey("speech_whisper_model", e.target.value)} aria-label="Whisper model">
            <option value="base.en">Base (lighter)</option>
            <option value="small.en">Small (more accurate)</option>
          </select>
          {st?.installing ? (
            <span className="wp-note wp-note--inline">{st.installing}</span>
          ) : (
            <button className="wp-btn" onClick={() => go(() => api.speechInstallWhisper(core.speech_whisper_model || "small.en"), "Downloaded. Choose Listen in the service bar to start.")}>
              {st?.whisper_models?.includes(core.speech_whisper_model || "small.en") ? "Download again" : "Download"}
            </button>
          )}
        </Row>
      )}
      {engine === "deepgram" && (
        <>
          <Row name="Deepgram API key" hint="From your account at deepgram.com (API Keys). Kept on this computer only, never synced or written to the log.">
            <input
              className="wp-input"
              type="password"
              autoComplete="off"
              placeholder="Paste the API key"
              defaultValue={core.deepgram_key ?? ""}
              onBlur={(e) => e.target.value !== (core.deepgram_key ?? "") && void setCoreKey("deepgram_key", e.target.value.trim())}
              aria-label="Deepgram API key"
            />
          </Row>
          <Row name="Deepgram model" hint="Nova-3 is the most accurate; Nova-2 is the older model.">
            <select className="wp-input" value={core.deepgram_model || "nova-3"} onChange={(e) => void setCoreKey("deepgram_model", e.target.value)} aria-label="Deepgram model">
              <option value="nova-3">Nova-3 (recommended)</option>
              <option value="nova-2">Nova-2</option>
            </select>
          </Row>
          <p className="wp-note">The preacher's microphone is sent to Deepgram over the internet while listening. Check Deepgram's pricing for your account.</p>
        </>
      )}
      {engine === "vosk" && (
        <Row name="Vosk engine and English model" hint={st?.vosk_ready ? "Downloaded and ready." : "Needs internet once, about 60 MB."}>
          {st?.installing ? (
            <span className="wp-note wp-note--inline">{st.installing}</span>
          ) : (
            <button className="wp-btn" onClick={() => go(api.speechInstallVosk, "Downloaded. Choose Listen in the service bar to start.")}>
              {st?.vosk_ready ? "Download again" : "Download"}
            </button>
          )}
        </Row>
      )}
      <Row name="Sermon transcript" hint="Everything heard since Listen was pressed, with times and the scriptures read: a start for sermon notes or social media posts.">
        <button
          className="wp-btn"
          onClick={async () => {
            const d = new Date();
            const path = await pickTextSave(`Sermon ${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}.txt`);
            if (path) go(() => api.speechSaveNotes(path), "Saved the sermon transcript.");
          }}
        >
          Save the sermon transcript…
        </button>
      </Row>
      {engine === "whisper" && (
        <Row name="Test with a recording" hint="Runs a sermon recording through Whisper and shows what would have been suggested, so accuracy can be checked and tuned.">
          <button
            className="wp-btn"
            disabled={!!st?.installing}
            onClick={async () => {
              const path = await pickRecording();
              if (!path) return;
              setTest(null);
              try {
                setTest(await api.speechTestRecording(path));
              } catch (e) {
                setTestProblem(String(e instanceof Error ? e.message : e));
              }
            }}
          >
            Choose a recording…
          </button>
        </Row>
      )}
      {test && (
        <div className="speech-test">
          <p className="wp-note">
            {test.found.length} reference{test.found.length === 1 ? "" : "s"} found in {Math.floor(test.seconds / 60)}:{String(test.seconds % 60).padStart(2, "0")} of speech (Whisper {test.model}, took {test.took_seconds} s).
          </p>
          <ul className="speech-test__found">
            {test.found.map(([t, r, how], i) => (
              <li key={i}>
                <span className="wp-num">{Math.floor(t / 60)}:{String(t % 60).padStart(2, "0")}</span> <strong>{r}</strong> <span className="wp-note wp-note--inline">{how}</span>
              </li>
            ))}
          </ul>
          <details>
            <summary>What Whisper heard</summary>
            {test.transcript.map(([t, s], i) => (
              <p key={i} className="speech-test__line">
                <span className="wp-num">{Math.floor(t / 60)}:{String(t % 60).padStart(2, "0")}</span> {s}
              </p>
            ))}
          </details>
        </div>
      )}
      {testProblem && <p className="form-error">{testProblem}</p>}
      {(engine === "vosk" || engine === "whisper" || engine === "deepgram") && (
        <Row name="Microphone" hint="The preacher's microphone, usually through the mixer's line or USB output.">
          <select className="wp-input" value={core.speech_mic ?? ""} onChange={(e) => void setCoreKey("speech_mic", e.target.value)}>
            <option value="">Windows' default microphone</option>
            {mics.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </Row>
      )}
      {engine === "vosk" && (
        <Row name="Listen for" hint="References only is far more accurate: Vosk listens only for book names, numbers and words like verse and chapter.">
          <select className="wp-input" value={refsOnly ? "refs" : "all"} onChange={(e) => void setCoreKey("speech_refs_only", e.target.value === "refs" ? "true" : "false")}>
            <option value="refs">Bible references only</option>
            <option value="all">Everything said</option>
          </select>
        </Row>
      )}
      {engine === "windows" && <p className="wp-note">Windows Speech uses Windows' default microphone (Windows sound settings, Recording).</p>}
      <Row name="Try it" hint="Read out a reference, e.g. “John three sixteen” or “first Corinthians thirteen verses four to seven”.">
        {st?.listening ? (
          <button className="wp-btn" onClick={() => go(api.speechStop)}>
            Stop
          </button>
        ) : (
          <button className="wp-btn" onClick={() => go(() => api.speechStart(engine, core.speech_mic || null, refsOnly))}>
            Start listening
          </button>
        )}
      </Row>
      {st?.listening && <p className="speech-caption">{st.caption ? `Hearing: ${st.caption}` : "Listening…"}</p>}
      {!!st?.recent.length && (
        <ul className="remote-log">
          {st.recent.slice(0, 8).map((r) => (
            <li key={r.reference + r.at}>
              <strong>{r.reference}</strong> from “{r.heard}”
            </li>
          ))}
        </ul>
      )}
      {(msg || st?.problem) && <p className={`wp-banner ${msg?.bad || (!msg && st?.problem) ? "wp-banner--bad" : ""}`}>{msg?.text ?? st?.problem}</p>}
    </>
  );
}

/** The church's colours and fonts, used by every theme linked to them. */
function BrandKit() {
  const [b, setB] = useState<Brand>(BRAND_DEFAULT);
  const [note, setNote] = useState<string | null>(null);
  useEffect(() => {
    api.getSettings().then((x) => setB(parseBrand(x))).catch(() => {});
  }, []);
  const save = async (n: Brand) => {
    setB(n);
    try {
      await api.setSetting("brand_colors", JSON.stringify(n.colors));
      await api.setSetting("brand_font", n.font);
      await api.setSetting("brand_heading_font", n.heading);
      setNote("Saved. Themes linked to the brand show the change everywhere.");
    } catch (e) {
      setNote(String(e instanceof Error ? e.message : e));
    }
  };
  return (
    <section>
      <h2>Brand kit</h2>
      <p className="wp-note">
        Your church’s colours and fonts. In the theme editor, click a brand swatch next to any colour (or choose “Brand font”) to link it; changing the brand here then changes every
        linked theme. Themes → right-click → “Re-colour to Brand” links a whole theme (handy for imported EasyWorship or ProPresenter themes).
      </p>
      {b.colors.map((c, i) => (
        <Row key={i} name={`${BRAND_NAMES[i]} colour`} hint={["Bars, boxes and highlights", "Accents, lines, choruses and second language", "The words", "Dark areas and backgrounds"][i]}>
          <input type="color" value={c} aria-label={`${BRAND_NAMES[i]} colour`} onChange={(e) => void save({ ...b, colors: b.colors.map((x, j) => (j === i ? e.target.value : x)) })} />
        </Row>
      ))}
      <Row name="Text font" hint="For lyrics and scripture">
        <FontPicker value={b.font} onChange={(font) => void save({ ...b, font })} />
      </Row>
      <Row name="Heading font" hint="For titles and section labels">
        <FontPicker value={b.heading} onChange={(heading) => void save({ ...b, heading })} />
      </Row>
      <div className="brand-sample" style={{ background: b.colors[3], color: b.colors[2], borderLeft: `6px solid ${b.colors[1]}` }}>
        <strong style={{ fontFamily: b.heading, color: b.colors[1] }}>MHB 1</strong>
        <span style={{ fontFamily: b.font }}>O for a thousand tongues to sing</span>
        <span style={{ background: b.colors[0], padding: "2px 8px", fontFamily: b.font }}>Meyi mo Pomfò ayɛw</span>
      </div>
      {note && <p className="wp-note">{note}</p>}
    </section>
  );
}

function HymnalPick({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [codes, setCodes] = useState<{ code: string; count: number }[]>([]);
  useEffect(() => {
    api.songTree().then((t) => setCodes(t.collections.map((c) => ({ code: c.key.split(":").pop() ?? c.name, count: c.count })))).catch(() => {});
  }, []);
  return (
    <select className="wp-input" value={value} onChange={(e) => onChange(e.target.value)} aria-label="Main hymnbook">
      <option value="">The biggest hymnbook{codes[0] ? ` (${[...codes].sort((a, b) => b.count - a.count)[0].code})` : ""}</option>
      {codes.map((c) => (
        <option key={c.code} value={c.code}>
          {c.code} ({c.count.toLocaleString()} songs)
        </option>
      ))}
    </select>
  );
}

/** A slide inviting the congregation to follow along: the QR code and the address. */
async function followSlide(url: string, svg: string) {
  const c = document.createElement("canvas");
  c.width = 1920;
  c.height = 1080;
  const g = c.getContext("2d")!;
  g.fillStyle = "#0d1424";
  g.fillRect(0, 0, 1920, 1080);
  g.fillStyle = "#ffffff";
  g.font = '800 88px "Noto Sans", sans-serif';
  g.fillText("Follow the words on your phone", 120, 200);
  g.font = '500 48px "Noto Sans", sans-serif';
  g.fillStyle = "#f2b705";
  g.fillText("English, Fante or both, in large print", 120, 290);
  g.fillStyle = "#dddddd";
  g.font = '500 44px "Noto Sans", sans-serif';
  g.fillText("1. Join the church Wi-Fi", 120, 470);
  g.fillText("2. Scan the code", 120, 550);
  g.font = '500 36px "Noto Sans", sans-serif';
  g.fillText(url, 120, 660);
  const img = new Image();
  img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
  await img.decode();
  g.fillStyle = "#ffffff";
  g.fillRect(1180, 260, 620, 620);
  g.drawImage(img, 1210, 290, 560, 560);
  const blob: Blob = await new Promise((ok) => c.toBlob((b) => ok(b!), "image/png"));
  const token = Date.now().toString(16);
  await api.presentationPage(token, 1, new Uint8Array(await blob.arrayBuffer()), "png");
  await api.presentationFinish(token, "Follow along on your phone");
}

const BUTTON_ACTIONS: Record<string, string> = {
  "go-live": "Go live",
  next: "Next slide",
  previous: "Previous slide",
  "next-item": "Next item",
  "previous-item": "Previous item",
  blank: "Blank",
  logo: "Logo",
  clear: "Clear text",
  resume: "Resume",
};

/** Stream Deck (through Bitfocus Companion) and MIDI pads as buttons. */
function ButtonsPanel() {
  const [key, setKey] = useState<string | null>(null);
  const [remote, setRemote] = useState<RemoteStatus | null>(null);
  const [ports, setPorts] = useState<string[]>([]);
  const [midi, setMidi] = useState<{ port: string | null; learning: string | null; map: Record<string, string> } | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const loadMidi = () => api.midiStatus().then(setMidi).catch(() => {});
  useEffect(() => {
    api.remoteControlKeyGet().then(setKey).catch(() => {});
    api.remoteStatus().then(setRemote).catch(() => {});
    api.midiPorts().then(setPorts).catch((e) => setMsg(String(e)));
    void loadMidi();
    const t = setInterval(loadMidi, 700);
    return () => clearInterval(t);
  }, []);
  const base = remote?.url ?? "http://THIS-COMPUTER:7300/";
  return (
    <section>
      <h2>Buttons and MIDI</h2>
      <h3>Stream Deck and Bitfocus Companion</h3>
      <p className="wp-note">
        Companion drives a Stream Deck (and many other button boxes). In Companion add a “Generic HTTP” connection and give each button one of the addresses below. The phone remote must be on (Settings → Phone remote).
      </p>
      <Row name="Control key" hint="Keeps other people on the network from pressing your buttons. Making a new key stops the old one.">
        <button className="wp-btn" onClick={async () => setKey(await api.remoteControlKey(true))}>
          {key ? "Make a new key" : "Make a key"}
        </button>
        {key && (
          <button className="wp-btn wp-btn--quiet" onClick={async () => setKey(await api.remoteControlKey(false))}>
            Switch off
          </button>
        )}
      </Row>
      {key && (
        <div className="btn-urls">
          {Object.entries(BUTTON_ACTIONS).map(([a, n]) => (
            <div key={a} className="btn-url">
              <span>{n}</span>
              <code>{`${base}api/v1/${a}?key=${key}`}</code>
            </div>
          ))}
          <div className="btn-url">
            <span>Service item 3</span>
            <code>{`${base}api/v1/item/3?key=${key}`}</code>
          </div>
        </div>
      )}
      <h3>MIDI pads and keyboards</h3>
      <Row name="MIDI device" hint="A pad controller, keyboard or mixer plugged into this computer.">
        <select className="wp-input" value={midi?.port ?? ""} onChange={(e) => void api.midiUse(e.target.value || null).then(loadMidi, (er) => setMsg(String(er)))} aria-label="MIDI device">
          <option value="">Off</option>
          {ports.map((p) => (
            <option key={p}>{p}</option>
          ))}
        </select>
      </Row>
      {midi?.port && (
        <div className="btn-urls">
          {Object.entries(BUTTON_ACTIONS).map(([a, n]) => (
            <div key={a} className="btn-url">
              <span>{n}</span>
              <span className="wp-num">{midi.learning === a ? "Press a pad…" : midi.map[a] ?? "Not set"}</span>
              <button className="btn-quiet" onClick={() => void api.midiLearn(a).then(loadMidi)}>
                Learn
              </button>
              {midi.map[a] && (
                <button className="btn-quiet" onClick={() => void api.midiForget(a).then(loadMidi)}>
                  Forget
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      {msg && <p className="form-error">{msg}</p>}
    </section>
  );
}

/** Keeping church computers alike through a shared folder. */
function SyncPanel() {
  type S = Awaited<ReturnType<typeof api.syncStatus>>;
  const [st, setSt] = useState<S | null>(null);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  const load = () => api.syncStatus().then(setSt).catch(() => {});
  useEffect(() => void load(), []);
  const when = (ms: number | null) => (ms ? new Date(ms).toLocaleString() : "never");
  const act = async (f: () => Promise<unknown>, ok: string) => {
    try {
      await f();
      setMsg({ text: ok });
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
    void load();
  };
  return (
    <section>
      <h2>Sync</h2>
      <p className="wp-note">
        Keeps church computers alike through a shared folder (OneDrive, Google Drive, a network folder or a USB drive). One computer shares its library; the others follow it. Shared:
        songs, Bibles, themes, media, presentations, services and collections. Each computer keeps its own settings and outputs (projectors, NDI, MIDI, microphone).
      </p>
      <Row name="Shared folder" hint={st?.folder ?? "Not chosen"}>
        <button className="wp-btn" onClick={async () => { const f = await pickFolder(); if (f) await act(() => api.syncSet(f, st?.role ?? null), "Folder chosen."); }}>
          Choose…
        </button>
      </Row>
      <Row name={`This computer (${st?.computer ?? ""})`} hint="Only one computer should share; the rest follow.">
        <select className="wp-input" value={st?.role ?? ""} onChange={(e) => void act(() => api.syncSet(st?.folder ?? null, e.target.value || null), "Saved.")} aria-label="This computer">
          <option value="">Does not sync</option>
          <option value="share">Shares its library (the main computer)</option>
          <option value="follow">Follows the main computer</option>
        </select>
      </Row>
      {st?.role === "share" && (
        <Row name="Share" hint={`Last shared: ${when(st.shared_ms)}. Also at every start and every 30 minutes.`}>
          <button className="wp-btn" disabled={!st.folder} onClick={() => void act(async () => setMsg({ text: `Shared (${await api.syncShareNow()} new media files).` }), "Shared.")}>
            Share now
          </button>
        </Row>
      )}
      {st?.role === "follow" && (
        <Row name="Update" hint={st.waiting ? `An update from ${st.waiting[0]} is ready (${when(st.waiting[1])}).` : `Up to date (last update: ${when(st.taken_ms)}). Updates are also taken at every start.`}>
          <button className="wp-btn wp-btn--primary" disabled={!st.waiting} onClick={() => void act(async () => setMsg({ text: `Updated from ${await api.syncTakeNow()}. A backup was made first.` }), "Updated.")}>
            Update now
          </button>
        </Row>
      )}
      {msg && <p className={msg.bad ? "form-error" : "wp-note"}>{msg.text}</p>}
    </section>
  );
}
