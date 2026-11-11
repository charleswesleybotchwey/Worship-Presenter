import { useEffect, useState } from "react";
import { api, pickCsvSave, type SpeechStatus, type NdiStatus, type RemoteStatus, type ServiceInterval, type UsageRow } from "../../api";
import { OutputOptions } from "./OutputOptions";
import { ACTION_NAMES, DEFAULT_KEYS, RESERVED, conflict, isSingleLetter, keyName, usePrefs, type Action } from "../../ui/prefs";
import type { OutputView } from "../../live/outputs";

// Settings (design brief §5): compact category list, one 680 px column of labelled
// rows with short explanations.

type Cat = "appearance" | "keyboard" | "projection" | "outputs" | "intervals" | "speech" | "remote" | "library" | "about";
const CATS: { id: Cat; label: string }[] = [
  { id: "appearance", label: "Appearance" },
  { id: "keyboard", label: "Keyboard" },
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
}: {
  uiTheme: "dark" | "light" | "system";
  setUiTheme: (t: "dark" | "light" | "system") => void;
  views: (role: string) => OutputView;
  mainLive: boolean;
  onOpenOutputs?: () => void;
  /** Open at this page (the Help menu's About and Keyboard shortcuts). */
  initial?: string;
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

          {cat === "library" && (
            <>
              <h2>Library</h2>
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
  const [st, setSt] = useState<SpeechStatus | null>(null);
  const [mics, setMics] = useState<string[]>([]);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  const engine = core.speech_engine || "vosk";
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
    ["vosk", "Vosk (recommended)", "Light and offline. Downloaded once (about 60 MB); then works without internet.", true],
    ["windows", "Windows Speech", "Offline, built into Windows; nothing to download. Less accurate in a noisy church.", !!st?.windows_speech],
    ["whisper", "Whisper", "Not available yet.", false],
    ["cloud", "A cloud service with your own key", "Not available yet.", false],
  ];
  return (
    <>
      <h2>Scripture recognition (experimental)</h2>
      <p className="wp-note">
        Listens to the preacher's microphone and suggests the Bible references it hears, above the Live pane. It never puts anything on the screens: you choose Preview or Go live. English now; Twi and Fante when speech models for them exist. Start and stop it with Listen in the service bar.
      </p>
      <Row name="Speech engine" hint="Which program turns speech into words.">
        <div className="speech-engines" role="radiogroup" aria-label="Speech engine">
          {engines.map(([id, name, hint, ok]) => (
            <label key={id} className={`speech-engine ${ok ? "" : "is-off"}`}>
              <input type="radio" name="engine" disabled={!ok} checked={engine === id} onChange={() => void setCoreKey("speech_engine", id)} />
              <span>
                <strong>{name}</strong>
                <small>{hint}</small>
              </span>
            </label>
          ))}
        </div>
      </Row>
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
      {engine === "vosk" && (
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
