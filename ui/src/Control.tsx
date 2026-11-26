import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { onControlAction, onMenu, onOutputConfigChanged, pickEwsOpen, slideForVerse, type Broadcast } from "./api";
import { StageView } from "./StageView";
import { msUntil, PreServicePanel, type PreServicePlan } from "./PreService";
import { OfferingSlide } from "./OfferingSlide";
import { OutlineDialog } from "./OutlineDialog";
import { printServiceOrder } from "./print";
import { RecordPanel, type RecStatus } from "./RecordPanel";
import { useMainAspect } from "./live/aspect";
import { TranscriptPanel } from "./TranscriptPanel";
import { LyricsFinder } from "./LyricsFinder";
import { FindReplace } from "./FindReplace";
import { OutputsPanel } from "./OutputsPanel";
import { PropsPanel } from "./PropsPanel";
import { MacrosPanel } from "./MacrosPanel";
import { PackageDialog, SongExportDialog } from "./PackageDialog";
import { langName } from "./languages";
import { LectionaryPanel } from "./LectionaryPanel";
import { GroupButtons } from "./features/service/GroupButtons";
import { TimelineBar } from "./features/service/TimelineBar";
import { TuneBar } from "./features/service/TuneBar";
import { defaultLook } from "./live/outputs";
import { ServiceActions, TimerChip, AutoBar, Clock, ViewToggle, PreviewMedia } from "./features/service/LiveControls";
import { GoLiveContext } from "./dnd";
import { api, isDesktop, type PulpitNote, type SpeechStatus, onLibraryRestored, onProfileChanged, onSettingsChanged, type LangMode, type LiveState, type Macro, type MediaItem, type Mode, type ProgramItem, type Schedule, type Slide } from "./api";
import { SlideView } from "./SlideView";
import { Icon } from "./Icon";
import { ServiceMenu, niceDate } from "./ServiceMenu";
import { ImportDialog } from "./ImportDialog";
import { SongEditor } from "./SongEditor";
import { BackupsDialog } from "./BackupsDialog";
import { ProfilesDialog } from "./ProfilesDialog";
import { ThemeEditor } from "./theme/ThemeEditor";
import { AlertPanel, TimerPanel } from "./CuePanels";
import { MusicPanel, useMusic } from "./MusicPanel";
import { CameraPanel } from "./CameraPanel";
import type { ThemeRow } from "./theme/types";
import { Palette, type PaletteCommand } from "./features/commands/Palette";
import { ContextMenu, SchedulePane, SlideGrid, itemTitle, sameItem, themeIdOf } from "./features/service/panes";
import { AnnouncementEditor } from "./features/editors/AnnouncementEditor";
import { LibraryDock, type LibTab } from "./features/library/LibraryDock";
import { SlideList } from "./features/service/SlideList";
import type { Deck } from "./api";
import { outputLabel, useLive, ROLE_NAMES, type OutputView } from "./live/outputs";
import { Settings } from "./features/editors/Settings";
import { OutputOptions } from "./features/editors/OutputOptions";
import { actionFor, usePrefs } from "./ui/prefs";

// The service window (design brief §3). Schedule is the sidebar; Library, Preview
// and Live are the work surface. Selection, typing, tabs and resizing never touch
// public output: only labelled service controls, their scoped shortcuts, and the
// Live pane do.

interface Loaded {
  item: ProgramItem;
  slides: Slide[];
  caption: string | null;
  media?: MediaItem | null;
}

type Tab = { id: "service" } | { id: "themes" } | { id: "settings" } | { id: "song"; songId: number | null } | { id: "ann"; deckId: number | null; template: Deck | null };

const MEDIA_SLIDE: Slide = { label: "Media", occurrence: 0, first_key: 0, last_key: 0, primary: [], second: [] };

function isTyping(el: EventTarget | null) {
  const t = el as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}

// ---------- UI theme: Dark, Light or Follow Windows ----------

type UiTheme = "dark" | "light" | "system";
function applyUiTheme(t: UiTheme) {
  const dark = t === "dark" || (t === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  // the Windows title bar and Mica follow the app
  void api.setWindowLook(dark).catch(() => {});
}
function useUiTheme(): [UiTheme, (t: UiTheme) => void] {
  const [t, setT] = useState<UiTheme>(() => (localStorage.getItem("wp-ui-theme") as UiTheme) || "dark");
  useEffect(() => {
    applyUiTheme(t);
    localStorage.setItem("wp-ui-theme", t);
    if (t !== "system") return;
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const f = () => applyUiTheme("system");
    mq.addEventListener("change", f);
    return () => mq.removeEventListener("change", f);
  }, [t]);
  return [t, setT];
}

function Splitter({ label, onDrag }: { label: string; onDrag: (dx: number) => void }) {
  return (
    <div
      className="wp-splitter"
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      tabIndex={0}
      onPointerDown={(e) => {
        e.preventDefault();
        let x = e.clientX;
        const move = (m: PointerEvent) => (onDrag(m.clientX - x), (x = m.clientX));
        const up = () => (window.removeEventListener("pointermove", move), window.removeEventListener("pointerup", up));
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
      }}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") e.preventDefault(), onDrag(-8);
        if (e.key === "ArrowRight") e.preventDefault(), onDrag(8);
      }}
    />
  );
}

function useWidth(ref: React.RefObject<HTMLElement>) {
  const [w, setW] = useState(1440);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setW(el.clientWidth));
    ro.observe(el);
    setW(el.clientWidth);
    return () => ro.disconnect();
  }, [ref]);
  return w;
}

// ---------- the window ----------

export function Control() {
  const { live, roles, view } = useLive();
  const [uiTheme, setUiTheme] = useUiTheme();
  const [prefs] = usePrefs();
  // Rehearsal: a simulator labelled Rehearsal; nothing reaches the screens (brief §6)
  const [rehearsal, setRehearsal] = useState(false);
  const [reh, setReh] = useState<{ item: ProgramItem; slides: Slide[]; caption: string | null; media?: MediaItem | null; slide_idx: number; mode: Mode } | null>(null);
  const [tabs, setTabs] = useState<Tab[]>([{ id: "service" }]);
  const [activeTab, setActiveTab] = useState<Tab["id"]>("service");
  const [schedule, setSchedule] = useState<ProgramItem[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [preview, setPreview] = useState<Loaded | null>(null);
  const [previewIdx, setPreviewIdx] = useState(0);
  const [libTab, setLibTab] = useState<LibTab>("songs");
  const [, setSourceTab] = useState<"schedule" | "library">("schedule");
  const [, setCompactTask] = useState<"schedule" | "library" | "preview">("preview");
  const [notice, setNotice] = useState<{ text: string; bad?: boolean; action?: { label: string; run: () => void } } | null>(null);
  const [layer, setLayer] = useState<null | "palette" | "outputs" | "import" | "backups" | "profiles" | "service" | "timer" | "alert" | "music" | "camera" | "preservice" | "offering" | "outline" | "record" | "lyrics" | "replace" | "outputs" | "dashboard" | "props" | "macros" | "package-export" | "package-import" | "lectionary">(null);
  const music = useMusic();
  const [profile, setProfile] = useState({ slug: "default", name: "" });
  const [libraryVersion, setLibraryVersion] = useState(0);
  const [themeRows, setThemeRows] = useState<ThemeRow[]>([]);
  const [stageNote, setStageNote] = useState("");
  const [advanceAfterLive, setAdvanceAfterLive] = useState(false);
  // ---------- scripture recognition (experimental; never goes live on its own) ----------
  const [speech, setSpeech] = useState<SpeechStatus | null>(null);
  const [speechCfg, setSpeechCfg] = useState({ engine: "vosk", mic: "", refsOnly: true });
  const [transcriptOpen, setTranscriptOpen] = useState(false);
  // short messages from other parts (Settings switching the speech engine)
  useEffect(() => {
    const f = (e: Event) => say(String((e as CustomEvent).detail));
    window.addEventListener("wp-toast", f);
    return () => window.removeEventListener("wp-toast", f);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  // go live once the item is in Preview: from slide idx, or from the slide labelled start
  // (a verse "3:16" in a whole chapter)
  const pendingLive = useRef<{ item: ProgramItem; idx: number; start?: string } | null>(null);
  const [notes, setNotes] = useState<PulpitNote[]>([]);
  useEffect(() => {
    const load = () =>
      api
        .getSettings()
        .then((x) => setSpeechCfg({ engine: x.speech_engine || "whisper", mic: x.speech_mic || "", refsOnly: x.speech_refs_only !== "false" }))
        .catch(() => {});
    load();
    let stop = () => {};
    onSettingsChanged(load).then((u) => (stop = u));
    return () => stop();
  }, []);
  useEffect(() => {
    let alive = true;
    // only store what changed: a fresh object every 0.7 s redrew the whole window
    let lastSpeech = "";
    let lastNotes = "";
    const tick = () => {
      api
        .speechStatus()
        .then((s) => {
          const k = JSON.stringify(s);
          if (alive && k !== lastSpeech) (lastSpeech = k), setSpeech(s);
        })
        .catch(() => {});
      api
        .pulpitNotes()
        .then((n) => {
          const k = JSON.stringify(n);
          if (alive && k !== lastNotes) (lastNotes = k), setNotes(n);
        })
        .catch(() => {});
    };
    tick();
    const t = setInterval(tick, 700);
    return () => ((alive = false), clearInterval(t));
  }, []);
  useEffect(() => {
    const load = () => api.getSettings().then((x) => setAdvanceAfterLive(x.advance_after_go_live === "true")).catch(() => {});
    load();
    let stop = () => {};
    onSettingsChanged(load).then((u) => (stop = u));
    return () => stop();
  }, []);
  const [service, setService] = useState<Omit<Schedule, "items"> | null>(null);
  const [saved, setSaved] = useState<"saved" | "saving" | "failed" | null>(null);
  const [lastBackup, setLastBackup] = useState<string | null>(null);
  const [quick, setQuick] = useState("");
  const loadedItems = useRef<string>("[]");
  const songSearch = useRef<HTMLInputElement>(null);
  const refInput = useRef<HTMLInputElement>(null);
  const quickInput = useRef<HTMLInputElement>(null);
  const liveSurface = useRef<HTMLDivElement>(null);
  const shell = useRef<HTMLDivElement>(null);
  const width = useWidth(shell);
  // EasyWorship layout: schedule width, preview/live split and top height, per profile
  const wide = width >= 1100;
  const ewRef = useRef<HTMLDivElement>(null);
  const ewKey = `wp-ew:${profile.slug}`;
  const [ew, setEw] = useState<{ schedule: number; split: number; top: number }>(() => {
    try {
      return { schedule: 240, split: 0.5, top: 58, ...JSON.parse(localStorage.getItem("wp-ew:default") ?? "{}") };
    } catch {
      return { schedule: 240, split: 0.5, top: 58 };
    }
  });
  const saveEw = (v: typeof ew) => (setEw(v), localStorage.setItem(ewKey, JSON.stringify(v)));
  const panes = { reset: () => (localStorage.removeItem(ewKey), setEw({ schedule: 240, split: 0.5, top: 58 })) };
  // Preview and Live each keep their own list/pictures choice
  const [previewView, setPreviewViewRaw] = useState<"list" | "thumbs">(() => (localStorage.getItem("wp-preview-view") as "list" | "thumbs") || (localStorage.getItem("wp-slide-view") as "list" | "thumbs") || "list");
  const [liveView, setLiveViewRaw] = useState<"list" | "thumbs">(() => (localStorage.getItem("wp-live-view") as "list" | "thumbs") || (localStorage.getItem("wp-slide-view") as "list" | "thumbs") || "list");
  const setPreviewView = (v: "list" | "thumbs") => (setPreviewViewRaw(v), localStorage.setItem("wp-preview-view", v));
  const setLiveView = (v: "list" | "thumbs") => (setLiveViewRaw(v), localStorage.setItem("wp-live-view", v));

  const state = live?.state;
  const liveSlides = live?.slides ?? [];

  // ---------- macros (as ProPresenter): several actions in order ----------
  const stateRef = useRef(state);
  stateRef.current = state;
  const runMacro = useCallback(async (m: Macro) => {
    let props = [...(stateRef.current?.props ?? [])];
    const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
    try {
      for (const a of m.actions) {
        const v = a.value;
        switch (a.kind) {
          case "blank":
          case "logo":
          case "clear":
            await api.setMode(a.kind);
            break;
          case "resume":
            await api.setMode("live");
            break;
          case "media_off":
          case "media_on":
            await api.setMediaOff(a.kind === "media_off");
            break;
          case "prop_on":
            props = [...props.filter((x) => x !== Number(v)), Number(v)];
            await api.setProps(props);
            break;
          case "prop_off":
            props = props.filter((x) => x !== Number(v));
            await api.setProps(props);
            break;
          case "props_clear":
            props = [];
            await api.setProps([]);
            break;
          case "next_item":
          case "previous_item":
            menuRef.current(a.kind === "next_item" ? "live.nextitem" : "live.previousitem");
            await sleep(400);
            break;
          case "countdown":
            await api.timerStart("Starting in", "countdown", `${Number(v) || 5}:00`, "corner", true);
            break;
          case "timer_stop":
            await api.timerControl("stop");
            break;
          case "music_play":
            await api.musicPlay(null);
            break;
          case "music_stop":
            await api.musicStop();
            break;
          case "look": {
            const t = v as { role?: string; look?: string };
            if (t?.role) {
              const c = await api.outputConfig(t.role);
              await api.saveOutputConfig({ ...c, look: t.look ?? "full" });
            }
            break;
          }
          case "clear_all":
            props = [];
            await api.clearAll();
            break;
          case "wait":
            await sleep(Math.min(60, Number(v) || 2) * 1000);
            break;
        }
      }
      say(`Ran “${m.name}”.`);
    } catch (e) {
      say(`“${m.name}” stopped: ${String(e instanceof Error ? e.message : e)}`, true);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const runMacroRef = useRef<(id: number) => Promise<void>>(async () => {});
  const runMacroId = useCallback(async (id: number) => {
    const m = (await api.macrosList()).find((x) => x.id === id);
    if (m) await runMacro(m);
    else say(`There is no macro number ${id}.`, true);
  }, [runMacro]);
  runMacroRef.current = runMacroId;

  // ---------- slide actions: a macro when a slide goes live ----------
  const slideKey = (it: ProgramItem | null | undefined, slides: Slide[], i: number): [string, number, string] | null => {
    if (it?.kind === "song") return ["song", it.song_id, slides[i]?.label ?? ""];
    if (it?.kind === "announcement") return ["announcement", it.presentation_id, String(i + 1)];
    return null;
  };
  const [liveActions, setLiveActions] = useState<Map<string, number>>(new Map());
  const [previewActions, setPreviewActions] = useState<Map<string, number>>(new Map());
  const liveItemKey = live?.item ? JSON.stringify(slideKey(live.item, [], 0)?.slice(0, 2) ?? null) : "";
  useEffect(() => {
    const k = slideKey(live?.item, [], 0);
    if (!k) return void setLiveActions(new Map());
    api.slideActions(k[0], k[1]).then((l) => setLiveActions(new Map(l))).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [liveItemKey]);
  const fired = useRef("");
  useEffect(() => {
    if (!state || state.mode !== "live" || rehearsal) return;
    const k = slideKey(live?.item, liveSlides, state.slide_idx);
    const id = k && liveActions.get(k[2]);
    const mark = `${liveItemKey}:${state.slide_idx}`;
    if (id && fired.current !== mark) {
      fired.current = mark;
      void runMacroId(id);
    } else if (!id) fired.current = mark;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state?.slide_idx, state?.mode, liveItemKey, liveActions]);
  const [slideMenu, setSlideMenu] = useState<{ x: number; y: number; i: number } | null>(null);
  const [toolsOpen, setToolsOpen] = useState(false);
  const [exportIds, setExportIds] = useState<number[] | null>(null);
  // after a power cut: what was on the screens, to pick up again
  const [recovery, setRecovery] = useState<{ schedule_id: number | null; item_idx: number; slide_idx: number } | null>(null);
  useEffect(() => {
    api.recoveryInfo().then(setRecovery).catch(() => {});
  }, []);
  // the live song's languages, for the language switch
  const [liveLangs, setLiveLangs] = useState<string[]>([]);
  const liveSongId = live?.item?.kind === "song" ? live.item.song_id : null;
  useEffect(() => {
    if (liveSongId == null) return void setLiveLangs([]);
    api.getSong(liveSongId).then((d) => setLiveLangs(d.lyrics.map((l) => l.lang))).catch(() => setLiveLangs([]));
  }, [liveSongId]);
  const previewItemKey = preview?.item ? JSON.stringify(slideKey(preview.item, [], 0)?.slice(0, 2) ?? null) : "";
  useEffect(() => {
    const k = slideKey(preview?.item, [], 0);
    if (!k) return void setPreviewActions(new Map());
    api.slideActions(k[0], k[1]).then((l) => setPreviewActions(new Map(l))).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [previewItemKey]);
  const [macroList, setMacroList] = useState<Macro[]>([]);
  const main = view("main");
  // an output set to "NDI Stream" has no window: say so instead of "No main display"
  const [mainNdi, setMainNdi] = useState(false);
  useEffect(() => {
    const load = () => api.outputConfig("main").then((c) => setMainNdi(!!c.ndi)).catch(() => {});
    load();
    let stop = () => {};
    onOutputConfigChanged(() => load()).then((u) => (stop = u));
    return () => stop();
  }, []);
  const mainLabel = mainNdi ? `Main: NDI stream${state?.mode && state.mode !== "live" ? ` (${state.mode})` : ""}` : outputLabel(main, state?.mode);
  const coreLatched = !!live?.item && !!state && state.mode !== "live";
  const latched = rehearsal ? !!reh && reh.mode !== "live" : coreLatched;
  const held = live?.held ?? [];
  // like EasyWorship, Go live works with no projector too (it shows in Live)
  // Go live always works: from Blank, Logo, Clear or a restart it brings the screens back
  // with the new item (as EasyWorship and FreeShow do); Resume returns without changing it.
  const canGoLive = !!preview;

  const say = (text: string, bad = false) => setNotice({ text, bad });
  // the service bar takes two rows when its buttons do not fit on one (measured, so it
  // keeps working as buttons are added): Go live, Blank, Logo and Clear never fall off
  const barRef = useRef<HTMLElement | null>(null);
  const oneRowNeeds = useRef(0);
  const [twoRows, setTwoRows] = useState(false);
  useLayoutEffect(() => {
    const bar = barRef.current;
    if (!bar) return;
    if (!twoRows && bar.scrollWidth > bar.clientWidth + 1) {
      oneRowNeeds.current = bar.scrollWidth;
      setTwoRows(true);
    } else if (twoRows && oneRowNeeds.current && bar.clientWidth >= oneRowNeeds.current + 8) {
      setTwoRows(false);
    }
  });
  // the service being saved (kept apart from the screen state, which arrives later)
  const serviceIdRef = useRef<number | null>(null);
  const run = useCallback(async <T,>(p: Promise<T>) => {
    try {
      const r = await p;
      setNotice(null);
      return r;
    } catch (e) {
      setNotice({ text: String(e instanceof Error ? e.message : e), bad: true });
      return undefined;
    }
  }, []);

  // ---------- service order, recovery, profiles ----------

  // an EasyWorship 7 schedule (.ewsx) becomes a new service here
  const openEws = async () => {
    const path = await pickEwsOpen();
    if (!path) return;
    const r = await run(api.importEws(path));
    if (!r) return;
    const today = new Date().toISOString().slice(0, 10);
    const created = await run(api.newSchedule(r.title, today, false, null));
    if (!created) return;
    await run(api.saveSchedule(created.id, r.items));
    openService({ ...created, items: r.items });
    setLibraryVersion((v) => v + 1);
    say(`Opened “${r.title}” from EasyWorship: ${r.items.length} items.${r.notes.length ? " " + r.notes.join(" ") : ""}`);
  };
  const openService = useCallback((sc: Schedule) => {
    const { items, ...meta } = sc;
    loadedItems.current = JSON.stringify(items);
    setService(meta);
    setSchedule(items);
    setSelected(null);
    setSaved(null);
  }, []);

  const afterRestore = useCallback(() => {
    loadedItems.current = "[]";
    serviceIdRef.current = null; // a new service: never save into the previous one
    setService(null);
    setSchedule([]);
    setPreview(null);
    setSaved(null);
    setLibraryVersion((v) => v + 1);
  }, []);

  const reopenLast = useCallback(() => {
    api.currentProfile().then(setProfile).catch(() => {});
    api.listBackups().then((b) => setLastBackup(b[0]?.taken ?? null)).catch(() => {});
    api
      .lastSchedule()
      .then((id) => (id != null ? api.openSchedule(id).then(openService) : undefined))
      .catch(() => {});
  }, [openService]);
  useEffect(() => reopenLast(), [reopenLast]);
  useEffect(() => {
    let a = () => {},
      b = () => {};
    onLibraryRestored(afterRestore).then((u) => (a = u));
    onProfileChanged(() => (afterRestore(), reopenLast())).then((u) => (b = u));
    return () => (a(), b());
  }, [afterRestore, reopenLast]);

  // Autosave, one save at a time: edits made while a save is running wait for it, and only
  // the newest waiting change is written (an older save can never land after a newer one;
  // a new service is created once, however fast the edits come)
  const saveChain = useRef<Promise<void>>(Promise.resolve());
  const waitingSave = useRef<{ json: string; items: ProgramItem[] } | null>(null);
  const serviceId = serviceIdRef;
  if (service?.id != null) serviceId.current = service.id;
  const flushSave = useCallback(() => {
    saveChain.current = saveChain.current.then(async () => {
      const job = waitingSave.current;
      if (!job) return;
      waitingSave.current = null;
      try {
        setSaved("saving");
        let id = serviceId.current;
        if (id == null) {
          const today = new Date().toISOString().slice(0, 10);
          const created = await api.newSchedule("Untitled service", today, false, null);
          const { items: _ignored, ...meta } = created;
          serviceId.current = created.id;
          setService(meta);
          id = created.id;
        }
        await api.saveSchedule(id, job.items);
        loadedItems.current = job.json;
        setSaved(waitingSave.current ? "saving" : "saved");
      } catch {
        setSaved("failed");
      }
    });
    return saveChain.current;
  }, []);
  useEffect(() => {
    const json = JSON.stringify(schedule);
    if (json === loadedItems.current) return;
    if (service == null && schedule.length === 0) return;
    waitingSave.current = { json, items: schedule };
    const t = setTimeout(() => void flushSave(), 400);
    return () => clearTimeout(t);
  }, [schedule, service, flushSave]);
  // closing the window right after an edit: write it now rather than lose it
  useEffect(() => {
    const now = () => void flushSave();
    window.addEventListener("beforeunload", now);
    return () => window.removeEventListener("beforeunload", now);
  }, [flushSave]);

  useEffect(() => {
    api.listThemes().then(setThemeRows).catch(() => {});
  }, [live?.theme_rev, activeTab]);

  // ---------- staging (never public) ----------

  const load = useCallback(
    async (item: ProgramItem, start?: string) => {
      // a heading in the service order has nothing to show
      if (item.kind === "header") return;
      if (item.kind === "song" && item.lyrics?.length) {
        // this service's own words
        const slides = await run(api.previewSongDoc({ id: item.song_id, title: item.title, author: null, copyright: null, ccli: null, numbers: [], lyrics: item.lyrics, sequence: "", lines_per_slide: null }));
        if (slides) setPreview({ item, slides, caption: item.title });
      } else if (item.kind === "song") {
        const slides = await run(api.previewSong(item.song_id, item.title));
        if (slides) setPreview({ item, slides, caption: item.title });
      } else if (item.kind === "announcement") {
        const slides = await run(api.previewAnnouncement(item.presentation_id));
        if (slides) setPreview({ item, slides, caption: item.title });
      } else if (item.kind === "media") {
        const all = await run(api.listMedia(null, ""));
        const media = all?.find((m) => m.id === item.media_id) ?? null;
        if (!media) return void setNotice({ text: "That picture or video is no longer in the library.", bad: true });
        setPreview({ item, slides: [MEDIA_SLIDE], caption: item.title, media });
      } else if (item.kind === "web") {
        // a web page: shown in its own window over the main screen when live
        setPreview({ item, slides: [{ label: "Web page", occurrence: 0, first_key: 0, last_key: 0, primary: [item.title], second: [item.url] }], caption: item.title });
      } else {
        const r = await run(api.previewScripture(item.reference, item.versions));
        if (r) {
          setPreview({ item, slides: r[0], caption: r[1] });
          // a whole chapter, positioned on the verse chosen
          const at = start ? slideForVerse(r[0], start) : 0;
          setPreviewIdx(Math.max(0, at));
          setCompactTask("preview");
          return;
        }
      }
      setPreviewIdx(0);
      setCompactTask("preview");
    },
    [run],
  );

  // Recovery: after a restart the last live cue comes back into Preview, not onto the screens.
  const recovered = useRef(false);
  useEffect(() => {
    if (recovered.current || !live) return;
    recovered.current = true;
    if (live.item && live.state.mode !== "live") {
      const item = live.item;
      void load(item).then(() => say(`Recovered “${itemTitle(item)}” into Preview at ${new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}. The screens stay as they are until you choose Resume output or Go live.`));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [live]);

  const setLook = (themeId: number | null) => {
    if (!preview || (preview.item.kind !== "song" && preview.item.kind !== "scripture")) return;
    const item = { ...preview.item, theme_id: themeId } as ProgramItem;
    setPreview({ ...preview, item });
    if (selected != null) setSchedule((sc) => sc.map((it, i) => (i === selected && sameItem(it, item) ? item : it)));
  };

  // ---------- transmission (explicit only) ----------

  const goLive = useCallback(async (at?: number) => {
    if (!preview) return;
    const slideAt = at ?? previewIdx;
    if (rehearsal) {
      setReh({ item: preview.item, slides: preview.slides, caption: preview.caption, media: preview.media, slide_idx: slideAt, mode: "live" });
      setTimeout(() => liveSurface.current?.focus(), 0);
      return;
    }
    if (!canGoLive) {
      if (main.condition === "none") return say("There is no main display yet. Use Set up output.", true);
      return;
    }
    const r = await run(api.goLive(preview.item, slideAt));
    // after Go live the keyboard drives Live (brief §6)
    if (r) setTimeout(() => liveSurface.current?.focus(), 0);
    // EasyWorship's "Advance schedule after Go Live": the next item comes into Preview
    if (r && advanceAfterLive && selected != null && sameItem(schedule[selected], preview.item) && selected + 1 < schedule.length) {
      const next = selected + 1;
      setSelected(next);
      void load(schedule[next]);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preview, previewIdx, canGoLive, main.condition, latched, run, rehearsal, advanceAfterLive, selected, schedule]);

  // Blank, Logo and Clear are switches: pressed again, the words come back
  const toggleMode = (m: Mode) => setMode((rehearsal ? reh?.mode : state?.mode) === m ? "live" : m);
  const setMode = (m: Mode) => (rehearsal ? (setReh((r) => (r ? { ...r, mode: m } : r)), Promise.resolve(undefined)) : run(api.setMode(m)));
  const resume = () => (rehearsal ? (setReh((r) => (r ? { ...r, mode: "live" } : r)), liveSurface.current?.focus()) : run(api.setMode("live")).then(() => liveSurface.current?.focus()));
  const step = (d: number) => {
    if (!rehearsal) return void run(api.step(d));
    setReh((r) => (r && r.mode === "live" ? { ...r, slide_idx: Math.max(0, Math.min(r.slides.length - 1, r.slide_idx + d)) } : r));
  };
  const show = (i: number) => {
    if (!rehearsal) return void run(api.show(i));
    setReh((r) => (r && r.mode === "live" ? { ...r, slide_idx: i } : r));
  };

  const stepSchedule = useCallback(
    async (d: number) => {
      if (!schedule.length) return;
      // headings are stepped over
      let next = (selected ?? -1) + d;
      while (next >= 0 && next < schedule.length && schedule[next].kind === "header") next += d;
      if (next < 0 || next >= schedule.length) return;
      setSelected(next);
      await load(schedule[next]);
    },
    [schedule, selected, load],
  );

  // ---------- keyboard scopes (brief §6) ----------

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Ctrl+P prints the service order (the browser's own print is switched off)
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "p" && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        return menuRef.current("file.print-order");
      }
      if (e.isComposing) return;
      const k = e.key;
      const typing = isTyping(e.target);
      const overlay = layer !== null;
      if (e.ctrlKey && k.toLowerCase() === "k") {
        if (!overlay) e.preventDefault(), setLayer("palette");
        return;
      }
      if (k === "F5") {
        e.preventDefault(); // never reload the operator window
        if (e.repeat || typing || overlay || activeTab !== "service") return;
        return void goLive();
      }
      if (activeTab !== "service" || overlay) return;
      if (e.ctrlKey && k.toLowerCase() === "f") return e.preventDefault(), setLibTab("songs"), setSourceTab("library"), setCompactTask("library"), setTimeout(() => songSearch.current?.focus());
      if (e.ctrlKey && k.toLowerCase() === "g") return e.preventDefault(), setTimeout(() => quickInput.current?.focus());
      if (k === "F6") {
        e.preventDefault();
        const regions = [...document.querySelectorAll<HTMLElement>("[data-region]")];
        const i = regions.findIndex((r) => r.contains(document.activeElement));
        const next = regions[(i + (e.shiftKey ? -1 : 1) + regions.length) % regions.length];
        (next?.querySelector<HTMLElement>(".wp-live-surface, [tabindex='0'], input, button") ?? next)?.focus();
        return;
      }
      // Esc leaves the field and does nothing else (one key, one effect)
      if (typing) {
        if (k === "Escape") e.preventDefault(), (e.target as HTMLElement).blur();
        return;
      }
      // Arrow and page keys drive Live from anywhere you are not typing (as in
      // EasyWorship); a held key still does not race through a hymn.
      const nav = ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp", "PageDown", "PageUp"].includes(k);
      const target = e.target as HTMLElement | null;
      const ownsArrows = !!target?.closest?.("input[type=radio], [role=slider], [role=listbox], [role=grid], input[type=range]");
      if (nav && !ownsArrows && !e.ctrlKey && !e.altKey && !e.metaKey) {
        if (e.repeat) return e.preventDefault();
        const map: Record<string, "next" | "previous" | "nextItem" | "previousItem"> = { ArrowRight: "next", ArrowDown: "next", ArrowLeft: "previous", ArrowUp: "previous", PageDown: "nextItem", PageUp: "previousItem" };
        const a = actionFor(prefs, k) ?? map[k];
        e.preventDefault();
        if (a === "next") return step(1);
        if (a === "previous") return step(-1);
        if (a === "nextItem") return void stepSchedule(1);
        if (a === "previousItem") return void stepSchedule(-1);
      }
      // Blank, Logo, Clear and the number keys only on the Live monitor itself, so a
      // stray letter elsewhere never changes the screens.
      if (!liveSurface.current || document.activeElement !== liveSurface.current) return;
      if (e.repeat) return e.preventDefault(); // a held key must not step through a hymn
      if (e.ctrlKey && k.toLowerCase() === "z") return e.preventDefault(), rehearsal ? undefined : void run(api.undoGoLive());
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      const act = actionFor(prefs, k);
      if (act) {
        e.preventDefault();
        if (act === "next") return step(1);
        if (act === "previous") return step(-1);
        if (act === "nextItem") return void stepSchedule(1);
        if (act === "previousItem") return void stepSchedule(-1);
        return void toggleMode(act === "blank" ? "blank" : act === "logo" ? "logo" : "clear");
      }
      const slides = rehearsal ? reh?.slides ?? [] : liveSlides;
      if (/^[1-9]$/.test(k) && slides.length) {
        const idx = slides.findIndex((s) => s.occurrence === Number(k) - 1);
        if (idx >= 0) return e.preventDefault(), show(idx);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [goLive, liveSlides, layer, activeTab, run, stepSchedule, prefs, rehearsal, reh]);

  useEffect(() => {
    requestAnimationFrame(() => requestAnimationFrame(() => void api.showMainWindow().catch(() => {})));
  }, []);

  // The Windows menu bar: each choice runs the same action as the palette or a key.
  const [settingsPage, setSettingsPage] = useState<string | undefined>(undefined);
  useEffect(() => {
    const open = () => setLayer("offering");
    const outline = () => setLayer("outline");
    const lyrics = () => setLayer("lyrics");
    window.addEventListener("wp-lyrics", lyrics);
    const replace = () => setLayer("replace");
    window.addEventListener("wp-replace", replace);
    // the output dots open the dashboard (the output settings dialog is "outputs")
    const outputs = () => setLayer("dashboard");
    window.addEventListener("wp-outputs", outputs);
    const props = () => setLayer("props");
    window.addEventListener("wp-props", props);
    const macros = () => setLayer("macros");
    window.addEventListener("wp-macros", macros);
    const songExport = (e: Event) => setExportIds((e as CustomEvent<number[]>).detail);
    window.addEventListener("wp-song-export", songExport);
    window.addEventListener("wp-offering", open);
    window.addEventListener("wp-outline", outline);
    return () => (window.removeEventListener("wp-offering", open), window.removeEventListener("wp-outline", outline), window.removeEventListener("wp-lyrics", lyrics));
  }, []);
  // slides take the main output's shape (4:3 projector → 4:3 Preview, Live and editor)
  useMainAspect();
  // a newer version (checked quietly once a day; installed only when the operator says)
  const [update, setUpdate] = useState<{ version: string; notes: string | null } | null>(null);
  useEffect(() => {
    const today = new Date().toDateString();
    if (localStorage.getItem("wp-update-checked") === today) return;
    const t = setTimeout(() => {
      localStorage.setItem("wp-update-checked", today);
      // only a build that can update itself looks (no failed check in the log otherwise)
      api
        .updatesOn()
        .then((on) => (on ? api.updateCheck().then((u) => u && setUpdate(u)) : undefined))
        .catch(() => {});
    }, 8000);
    return () => clearTimeout(t);
  }, []);
  // recording / streaming status (a red timer in the service bar while running)
  const [rec, setRec] = useState<RecStatus | null>(null);
  useEffect(() => {
    let last = "";
    const tick = () =>
      api
        .streamStatus()
        .then((s) => {
          const k = JSON.stringify(s);
          if (k !== last) (last = k), setRec(s);
        })
        .catch(() => {});
    tick();
    const t = setInterval(tick, 1000);
    return () => clearInterval(t);
  }, []);
  // the pre-service loop: on time, the first item goes live (or the logo shows)
  const [preService, setPreService] = useState<PreServicePlan | null>(null);
  const scheduleRef = useRef(schedule);
  scheduleRef.current = schedule;
  useEffect(() => {
    if (!preService) return;
    const t = setInterval(() => {
      if (msUntil(preService.startsAt) > 0) return;
      clearInterval(t);
      setPreService(null);
      void api.timerControl("stop").catch(() => {});
      if (preService.onTime === "first" && scheduleRef.current[0]) {
        setSelected(0);
        pendingLive.current = { item: scheduleRef.current[0], idx: 0 };
        void load(scheduleRef.current[0]);
      } else if (preService.onTime === "logo") void setMode("logo");
      say("It is time: the pre-service loop has finished.");
    }, 1000);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preService]);
  // the theme the theme rules choose for the item in Preview
  const [previewRule, setPreviewRule] = useState<number | null>(null);
  const [rulesRev, setRulesRev] = useState(0);
  useEffect(() => {
    let stop = () => {};
    onSettingsChanged(() => setRulesRev((n) => n + 1)).then((u) => (stop = u));
    return () => stop();
  }, []);
  useEffect(() => {
    const it = preview?.item;
    if (!it || (it.kind !== "song" && it.kind !== "scripture")) return setPreviewRule(null);
    let alive = true;
    api.resolveTheme(it).then((id) => alive && setPreviewRule(id ?? null)).catch(() => alive && setPreviewRule(null));
    return () => {
      alive = false;
    };
  }, [preview?.item, rulesRev]);
  const [themeRequest, setThemeRequest] = useState<{ id?: number; newKind?: import("./theme/types").ThemeKind; n: number } | undefined>(undefined);
  const menuRef = useRef<(id: string) => void>(() => {});
  menuRef.current = (id: string) => {
    const palette = (pid: string) => commands.find((c) => c.id === pid)?.run();
    const toService = () => setActiveTab("service");
    switch (id) {
      case "file.service": return palette("service");
      case "file.import": return palette("import");
      case "file.ews": return void openEws();
      case "file.package-export": return setLayer("package-export");
      case "file.package-import": return setLayer("package-import");
      case "file.backups": return palette("backups");
      case "file.print-order": return palette("print-order");
      case "file.outputs": return palette("outputs");
      case "file.settings": return palette("settings");
      case "file.exit": return void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().close());
      case "edit.newsong": return palette("new-song");
      case "edit.find": return palette("song-search");
      case "edit.quick": return palette("quick");
      case "edit.palette": return setLayer("palette");
      case "live.go": return toService(), void goLive();
      case "live.next": return toService(), step(1);
      case "live.previous": return toService(), step(-1);
      case "live.nextitem": return toService(), void stepSchedule(1);
      case "live.previousitem": return toService(), void stepSchedule(-1);
      case "live.blank": return void toggleMode("blank");
      case "live.logo": return void toggleMode("logo");
      case "live.clear": return void toggleMode("clear");
      case "live.resume": return void setMode("live");
      case "live.timer": return palette("timer");
      case "live.alert": return palette("alert");
      case "live.music": return palette("music");
      case "live.rehearsal": return palette("rehearsal");
      case "profiles.manage": return palette("profiles");
      case "view.dark": return setUiTheme("dark");
      case "view.light": return setUiTheme("light");
      case "view.system": return setUiTheme("system");
      case "view.preview-list": return setPreviewView("list");
      case "view.preview-pictures": return setPreviewView("thumbs");
      case "view.live-list": return setLiveView("list");
      case "view.live-pictures": return setLiveView("thumbs");
      case "view.themes": return palette("themes");
      case "view.media": return palette("media");
      case "view.reset": return palette("reset-panes");
      case "help.shortcuts": return setSettingsPage("keyboard"), openTab({ id: "settings" });
      case "help.about": return setSettingsPage("about"), openTab({ id: "settings" });
    }
  };
  useEffect(() => onMenu((id) => menuRef.current(id)), []);
  // a sync update replaced the library: start fresh with it
  useEffect(() => {
    if (!isDesktop) return;
    let stop = () => {};
    import("@tauri-apps/api/event").then(({ listen }) => listen("library-updated", () => location.reload()).then((u) => (stop = u)));
    return () => stop();
  }, []);
  // Stream Deck / Companion / MIDI pads: actions that need what this window holds
  useEffect(() => {
    let stop = () => {};
    onControlAction((a) => (a.startsWith("macro:") ? void runMacroRef.current(Number(a.slice(6))) : menuRef.current(a === "go-live" ? "live.go" : a === "next-item" ? "live.nextitem" : "live.previousitem"))).then((u) => (stop = u));
    return () => stop();
  }, []);

  // Closing the window during a service asks first; cancelling leaves output unchanged.
  // Registered once (a fresh handler per state change leaked duplicates); reads the
  // latest state through a ref. The app then closes every window and exits.
  const closeInfo = useRef({ onAir: false });
  closeInfo.current.onAir = !!live?.item && state?.mode === "live";
  useEffect(() => {
    if (!isDesktop) return;
    let un = () => {};
    let gone = false;
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) =>
      getCurrentWindow()
        .onCloseRequested((ev) => {
          if (closeInfo.current.onAir && !confirm("Something is on the screens. Close Worship Presenter and end output?")) ev.preventDefault();
        })
        .then((u) => (gone ? u() : (un = u))),
    );
    return () => ((gone = true), un());
  }, []);

  // ---------- tabs ----------

  const openTab = (t: Tab) => {
    setTabs((ts) => (ts.some((x) => x.id === t.id) ? ts.map((x) => (x.id === t.id ? t : x)) : [...ts, t]));
    setActiveTab(t.id);
  };
  const closeTab = (id: Tab["id"]) => {
    setTabs((ts) => ts.filter((x) => x.id !== id));
    setActiveTab("service");
  };

  // ---------- commands (safe actions only) ----------

  const commands: PaletteCommand[] = useMemo(
    () => [
      { id: "song-search", group: "Library", label: "Search songs", hint: "Ctrl+F", run: () => (setActiveTab("service"), setLibTab("songs"), setSourceTab("library"), setTimeout(() => songSearch.current?.focus())) },
      { id: "quick", group: "Library", label: "Quick scripture", hint: "Ctrl+G", run: () => (setActiveTab("service"), setTimeout(() => quickInput.current?.focus())) },
      { id: "new-song", group: "Library", label: "New song", run: () => openTab({ id: "song", songId: null }) },
      { id: "lyrics", group: "Library", label: "Find lyrics online…", run: () => setLayer("lyrics") },
      { id: "replace", group: "Library", label: "Find and replace in songs…", run: () => setLayer("replace") },
      { id: "web", group: "Service", label: "Add a web page to the service…", run: () => document.querySelector<HTMLButtonElement>("button[aria-label='Add a web page']")?.click() },
      { id: "macros", group: "Live", label: "Macros…", run: () => setLayer("macros") },
      { id: "props", group: "Live", label: "Props (logo, LIVE badge, sermon title, clock)…", run: () => setLayer("props") },
      { id: "clear-all", group: "Live", label: "Clear all (words, media, props, alert, music)", run: () => void api.clearAll() },
      { id: "check", group: "Service", label: "Check system (screens, disk, backup…)", run: () => setLayer("dashboard") },
      { id: "outputs-panel", group: "Service", label: "Outputs dashboard", run: () => setLayer("dashboard") },
      { id: "liturgy", group: "Library", label: "Add creeds and prayers to the library (Apostles' Creed, Lord's Prayer…)", run: () => void api.liturgyAdd().then(([a, t]) => say(a ? `${a} creeds and prayers added to “Creeds and prayers”.` : `All ${t} are already in the library.`)) },
      { id: "lectionary", group: "Service", label: "Lectionary readings for this service…", run: () => setLayer("lectionary") },
      { id: "pkg-export", group: "Library", label: "Export a Worship Presenter package (songs in every language, themes, Bibles)…", run: () => setLayer("package-export") },
      { id: "pkg-import", group: "Library", label: "Import a Worship Presenter package…", run: () => setLayer("package-import") },
      { id: "ews", group: "Service", label: "Open an EasyWorship schedule (.ewsx)…", run: () => void openEws() },
      { id: "ann", group: "Library", label: "Announcements (offering, birthdays, welcome loop)", keywords: "notices slides momo tithe", run: () => (setActiveTab("service"), setLibTab("presentations"), setSourceTab("library")) },
      { id: "media", group: "Library", label: "Media library", run: () => (setActiveTab("service"), setLibTab("media"), setSourceTab("library")) },
      { id: "import", group: "Settings", label: "Import songs, Bibles and presentations", run: () => (setSettingsPage("import"), openTab({ id: "settings" })) },
      { id: "backups", group: "Settings", label: "Backups", run: () => (setSettingsPage("backups"), openTab({ id: "settings" })) },
      { id: "profiles", group: "Settings", label: "Profiles", run: () => (setSettingsPage("profiles"), openTab({ id: "settings" })) },
      { id: "service", group: "Service", label: "Open or start a service order…", run: () => setLayer("service") },
      { id: "offering", group: "Service", label: "Offering slide (MoMo, QR)…", run: () => setLayer("offering") },
      { id: "outline", group: "Service", label: "Sermon outline to slides…", run: () => setLayer("outline") },
      { id: "print-order", group: "Service", label: "Print the service order…", run: () => printServiceOrder(service?.name ?? "Service order", service?.service_date ?? null, schedule.map((i) => ({ title: itemTitle(i), kind: i.kind }))) },
      { id: "preservice", group: "Service", label: "Pre-service loop (countdown, announcements, music)…", run: () => setLayer("preservice") },
      { id: "timer", group: "Service", label: "Timer…", run: () => setLayer("timer") },
      { id: "alert", group: "Service", label: "Alert…", run: () => setLayer("alert") },
      { id: "music", group: "Service", label: "Background music…", keywords: "audio playlist songs instrumental", run: () => setLayer("music") },
      { id: "live", group: "Service", label: "Focus Live (keyboard drives the screens)", run: () => (setActiveTab("service"), setTimeout(() => liveSurface.current?.focus())) },
      { id: "outputs", group: "Settings", label: "Outputs and NDI", run: () => (setSettingsPage("outputs"), openTab({ id: "settings" })) },
      // themes are chosen in the library; the editor opens on one theme only
      { id: "themes", group: "Design", label: "Themes", run: () => (setActiveTab("service"), setLibTab("themes"), setSourceTab("library")) },
      { id: "settings", group: "Appearance", label: "Settings", keywords: "preferences keyboard shortcuts low spec", run: () => openTab({ id: "settings" }) },
      { id: "rehearsal", group: "Service", label: "Rehearsal mode on or off", keywords: "practice simulate", run: () => setRehearsal((r) => (setReh(null), !r)) },
      { id: "ui-dark", group: "Appearance", label: "Appearance: Dark", keywords: "theme colour", run: () => setUiTheme("dark") },
      { id: "ui-light", group: "Appearance", label: "Appearance: Light", keywords: "theme colour", run: () => setUiTheme("light") },
      { id: "ui-system", group: "Appearance", label: "Appearance: Follow Windows", keywords: "theme colour system", run: () => setUiTheme("system") },
      { id: "reset-panes", group: "Settings", label: "Reset pane sizes", run: () => panes.reset() },
    ],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [panes.reset],
  );

  const heardItem = async (ref: string): Promise<ProgramItem | null> => {
    // the versions of the reading in Preview, else the first Bible
    const v = preview?.item.kind === "scripture" ? preview.item.versions : [(await run(api.versions()))?.[0]?.code].filter(Boolean) as string[];
    if (!v.length) return say("No Bibles yet. Import one with Ctrl+K, Import.", true), null;
    return { kind: "scripture", reference: ref, versions: v };
  };
  const heardPreview = async (ref: string) => {
    const it = await heardItem(ref);
    if (it) await load(it);
  };
  const heardLive = async (ref: string) => {
    const it = await heardItem(ref);
    if (!it) return;
    pendingLive.current = { item: it, idx: 0 };
    await load(it);
  };
  // "Go live" on a suggestion: once it is in Preview, the usual Go live (and its checks) runs
  useEffect(() => {
    const p = pendingLive.current;
    if (p && preview && sameItem(preview.item, p.item)) {
      pendingLive.current = null;
      const idx = p.start ? slideForVerse(preview.slides, p.start) : p.idx;
      setPreviewIdx(idx);
      void goLive(idx);
    }
  }, [preview, goLive]);
  // ---------- notes from the pulpit phone ----------
  const noteItem = (n: PulpitNote): ProgramItem => ({ kind: "announcement", presentation_id: n.deck_id, title: n.deck_title });
  const notePreview = async (n: PulpitNote) => {
    await load(noteItem(n));
    setPreviewIdx(n.slide_idx);
  };
  const noteLive = async (n: PulpitNote) => {
    pendingLive.current = { item: noteItem(n), idx: n.slide_idx };
    await load(noteItem(n));
  };
  const latestNote = notes[0];
  const noteChip = latestNote && (
    <div className="heard heard--note" role="status" aria-live="polite" onKeyDown={(e) => e.key === "Escape" && (e.preventDefault(), void api.pulpitDismiss(latestNote.id))}>
      <Icon name="note" />
      <span className="heard__ref">
        Note from {latestNote.device}
        <strong className="heard__note">{latestNote.text}</strong>
      </span>
      <button className="wp-btn wp-btn--quiet" onClick={() => notePreview(latestNote)}>
        Preview
      </button>
      <button className="wp-btn wp-btn--primary" onClick={() => noteLive(latestNote)}>
        Go live
      </button>
      {notes.length > 1 && <span className="heard__more">+{notes.length - 1} more</span>}
      <button className="wp-icon-btn" aria-label="Dismiss this note" onClick={() => api.pulpitDismiss(latestNote.id)}>
        <Icon name="close" />
      </button>
    </div>
  );

  const heardDismiss = (ref: string) => {
    void api.speechDismiss(ref);
  };
  const toggleListening = async () => {
    if (speech?.listening) return void api.speechStop();
    try {
      // the choices as they are now (Settings may have changed them)
      const x = await api.getSettings().catch(() => ({}) as Record<string, string>);
      const cfg = { engine: x.speech_engine || speechCfg.engine, mic: x.speech_mic ?? speechCfg.mic, refsOnly: x.speech_refs_only !== "false" };
      setSpeechCfg(cfg);
      await api.speechStart(cfg.engine, cfg.mic || null, cfg.refsOnly);
      const names: Record<string, string> = { whisper: "Whisper", deepgram: "Deepgram", vosk: "Vosk", windows: "Windows Speech" };
      say(`Listening for Bible references (${names[cfg.engine] ?? cfg.engine}). Suggestions appear above Live; nothing goes live by itself.`);
    } catch (e) {
      say(String(e instanceof Error ? e.message : e), true);
    }
  };

  // ---------- quick scripture (stages only) ----------

  const quickStage = async () => {
    // a hymn number first ("555", "MHB 555"), else scripture
    const songs = await api.quickSong(quick).catch(() => []);
    if (songs.length) {
      const s = songs[0];
      await load({ kind: "song", song_id: s.id, title: s.title });
      return say(songs.length > 1 ? `${s.title} in Preview (also: ${songs.slice(1).map((x) => x.title).join(", ")})` : `${s.title} in Preview`);
    }
    const v = await run(api.versions());
    const code = v?.[0]?.code;
    if (!code) return say("No Bibles yet. Import one with Ctrl+K, Import.", true);
    await load({ kind: "scripture", reference: quick, versions: [code] });
  };

  // ---------- panes ----------


  const schedulePane = (
    <SchedulePane
      items={schedule}
      selected={selected}
      liveItem={live?.item ?? null}
      dateLabel={service ? `${service.template ? "Template" : service.service_date ? niceDate(service.service_date) : ""}` : ""}
      onSelect={(i) => (setSelected(i), void load(schedule[i]))}
      onThemeDrop={(i, themeId) => {
        const it = schedule[i];
        if (!it || (it.kind !== "song" && it.kind !== "scripture")) return say("Themes apply to songs and readings.", true);
        const item = { ...it, theme_id: themeId } as ProgramItem;
        setSchedule((sc) => sc.map((x, k) => (k === i ? item : x)));
        if (preview && sameItem(preview.item, it)) setPreview({ ...preview, item });
        // on the screens now: the new look at once, on the same slide
        if (live?.item && sameItem(live.item, it) && state?.mode === "live") void run(api.goLive(item, state.slide_idx));
        say(`“${itemTitle(it)}” now uses that theme.`);
      }}
      onWebPage={(i) => {
        const url = prompt("Web page address (https://…)", "https://");
        if (!url || !/^https?:\/\/\S+\.\S+/.test(url.trim())) return url && url !== "https://" ? say("That is not a web address (it starts with https://).", true) : undefined;
        const title = prompt("A name for it in the service order", new URL(url.trim()).hostname) || url.trim();
        const w: ProgramItem = { kind: "web", url: url.trim(), title };
        setSchedule((sc) => (i == null ? [...sc, w] : [...sc.slice(0, i + 1), w, ...sc.slice(i + 1)]));
      }}
      onHeading={(i, rename) => {
        const old = rename && i != null && schedule[i]?.kind === "header" ? (schedule[i] as Extract<ProgramItem, { kind: "header" }>).title : "";
        const title = prompt(rename ? "Rename the heading" : "Heading (e.g. Praise & Worship, Sermon, Offertory)", old);
        if (!title?.trim()) return;
        const h: ProgramItem = { kind: "header", title: title.trim() };
        setSchedule((sc) => (rename && i != null ? sc.map((x, k) => (k === i ? h : x)) : i == null ? [...sc, h] : [...sc.slice(0, i + 1), h, ...sc.slice(i + 1)]));
      }}
      onActivate={(i) => {
        setSelected(i);
        pendingLive.current = { item: schedule[i], idx: 0 };
        void load(schedule[i]);
      }}
      onRemove={(i) => setSchedule((s) => s.filter((_, j) => j !== i))}
      onMove={(from, to) =>
        setSchedule((s) => {
          const n = [...s];
          const [x] = n.splice(from, 1);
          n.splice(to, 0, x);
          return n;
        })
      }
      onAddFromLibrary={() => (setLibTab("songs"), setSourceTab("library"), setCompactTask("library"), setTimeout(() => songSearch.current?.focus()))}
      onMenu={() => setLayer("service")}
      onDropItem={(item, at) => setSchedule((sc) => (at == null ? [...sc, item] : [...sc.slice(0, at), item, ...sc.slice(at)]))}
    />
  );

  const libraryDock = (
    <GoLiveContext.Provider value={(item, start) => {
      pendingLive.current = { item, idx: 0, start };
      void load(item, start);
    }}>
    <LibraryDock
      onAir={{ item: rehearsal ? null : live?.item, label: liveSlides[state?.slide_idx ?? -1]?.label }}
      inPreview={{ item: preview?.item, label: preview?.slides[previewIdx]?.label }}
      tab={libTab}
      setTab={setLibTab}
      version={libraryVersion}
      songSearch={songSearch}
      refInput={refInput}
      previewKind={preview?.item.kind ?? null}
      onPreview={(item, start) => void load(item, start)}
      onAdd={(item) => setSchedule((sc) => [...sc, item])}
      onEditSong={(id) => openTab({ id: "song", songId: id })}
      onNewSong={() => openTab({ id: "song", songId: null })}
      onMediaPreview={(m) => (setPreview({ item: { kind: "media", media_id: m.id, title: m.title }, slides: [MEDIA_SLIDE], caption: m.title, media: m }), setPreviewIdx(0))}
      onEditDeck={(id) => openTab({ id: "ann", deckId: id, template: null })}
      onNewDeck={(template) => openTab({ id: "ann", deckId: null, template })}
      onUseTheme={(id) => setLook(id)}
      onEditThemes={(req) => (setThemeRequest(req ? { ...req, n: Date.now() } : undefined), openTab({ id: "themes" }))}
      extraTab={wide ? undefined : { label: "Schedule", node: schedulePane }}
    />
    </GoLiveContext.Provider>
  );

  const previewSlide = preview?.slides[previewIdx];
  const previewPane = (
    <section className="wp-pane wp-pane--work ew-pane" aria-label="Preview" data-region="preview">
      <header className="wp-pane__head">
        <h2>Preview</h2>
        <span className="wp-pane__sub" title={preview ? itemTitle(preview.item) : undefined}>
          {preview ? itemTitle(preview.item) : ""}
        </span>
        {preview && (preview.item.kind === "song" || preview.item.kind === "scripture") && (
          <label className="wp-inline-select">
            <span>Look</span>
            <select value={themeIdOf(preview.item) ?? ""} onChange={(e) => setLook(e.target.value ? Number(e.target.value) : null)}>
              <option value="">Theme in use</option>
              {themeRows
                .filter((t) => t.kind === (preview.item.kind === "scripture" ? "scripture" : "song"))
                .map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                    {t.is_default ? " (in use)" : ""}
                  </option>
                ))}
            </select>
          </label>
        )}
        <ViewToggle value={previewView} onChange={setPreviewView} />
      </header>
      {noteChip}
      <div className="ew-list">
        {!preview ? (
          <div className="wp-empty">
            <p>Select an item to preview.</p>
          </div>
        ) : previewView === "list" ? (
          <SlideList
            slides={preview.slides}
            current={previewIdx}
            onPick={setPreviewIdx}
            onActivate={(i) => (setPreviewIdx(i), void goLive(i))}
            empty="This item has no slides."
            marked={(i) => {
              const k = slideKey(preview.item, preview.slides, i);
              return !!k && previewActions.has(k[2]);
            }}
            onMenu={(i, x, y) => {
              if (!slideKey(preview.item, preview.slides, i)) return say("Slide actions are for songs and announcements.", true);
              api.macrosList().then(setMacroList).catch(() => {});
              setSlideMenu({ x, y, i });
            }}
          />
        ) : (
          <SlideGrid
            slides={preview.slides}
            kind={preview.item.kind}
            media={preview.media}
            themeId={themeIdOf(preview.item) ?? previewRule}
            current={previewIdx}
            onPick={setPreviewIdx}
            onActivate={(i) => (setPreviewIdx(i), void goLive(i))}
            empty="This item has no slides."
          />
        )}
      </div>
      <div className="ew-out">
        <div className="ew-out__mon" onDoubleClick={() => void goLive()} title="Double-click to go live">
          {preview?.media && preview.media.kind !== "image" ? (
            <PreviewMedia item={preview.media} />
          ) : (
            <SlideView slide={previewSlide} allSlides={preview?.slides} position={{ first: previewIdx === 0, last: previewIdx >= (preview?.slides.length ?? 1) - 1 }} kind={preview?.item.kind} caption={preview?.caption} media={preview?.media} themeId={themeIdOf(preview?.item) ?? previewRule} still />
          )}
        </div>
        <div className="ew-out__info">
          <span className="ew-out__label">Preview</span>
          <strong className="ew-out__title">{preview ? itemTitle(preview.item) : "Nothing staged"}</strong>
          {preview && (
            <span className="wp-num">
              Slide {previewIdx + 1} of {preview.slides.length}
            </span>
          )}
          {preview?.media && preview.media.kind !== "image" && <span className="wp-note wp-note--inline">Plays muted here. Sound goes to the main output when live.</span>}
          <div className="ew-out__nav">
            <button className="wp-icon-btn" aria-label="Previous preview slide" disabled={!preview || previewIdx === 0} onClick={() => setPreviewIdx((i) => Math.max(0, i - 1))}>
              ‹
            </button>
            <button className="wp-icon-btn" aria-label="Next preview slide" disabled={!preview || previewIdx >= (preview?.slides.length ?? 1) - 1} onClick={() => setPreviewIdx((i) => Math.min((preview?.slides.length ?? 1) - 1, i + 1))}>
              ›
            </button>
          </div>
        </div>
      </div>
    </section>
  );

  // The Live monitor shows what Main last confirmed, never just what was requested.
  // which output the Live picture shows (EasyWorship's Live Output / Alternate Output)
  const [monitorChoice, setMonitorRoleRaw] = useState<string>(() => localStorage.getItem("wp-live-monitor") || "main");
  const setMonitorRole = (r: string) => (setMonitorRoleRaw(r), localStorage.setItem("wp-live-monitor", r));
  // every screen can be watched here; it is drawn as its look (full, lower third, stage…)
  const [screens, setScreens] = useState<{ role: string; name: string; look?: string }[]>([]);
  useEffect(() => {
    const load = () => api.outputConfigs().then(setScreens).catch(() => {});
    load();
    let off = () => {};
    onOutputConfigChanged(() => load()).then((u) => (off = u));
    return () => off();
  }, []);
  const watchedLook = screens.find((x) => x.role === monitorChoice)?.look ?? defaultLook(monitorChoice);
  const monitorRole: "main" | "alternate" | "foldback" = watchedLook === "stage" ? "foldback" : watchedLook === "lower" ? "alternate" : "main";
  const conf = rehearsal
    ? reh
      ? ({ item: reh.item, slides: reh.slides, caption: reh.caption, media: reh.media ?? null, state: { ...(state ?? ({} as LiveState)), slide_idx: reh.slide_idx, mode: reh.mode } } as unknown as typeof live)
      : null
    : main.condition === "none"
      ? live
      : main.confirmed ?? null;
  const confSlides = conf?.slides ?? [];
  const confState = conf?.state;
  const confSlide = confSlides[confState?.slide_idx ?? 0];
  const liveTitle = conf?.item ? `${itemTitle(conf.item)}${confSlide && conf.item.kind !== "media" ? " · " + confSlide.label : ""}` : "Nothing on the screens";
  const stateClass = rehearsal ? "rehearsal" : main.condition === "confirmed" && confState?.mode === "live" && conf?.item ? "on" : main.condition === "confirmed" ? "idle" : main.condition;
  const liveLabel = rehearsal ? `Rehearsal${reh && reh.mode !== "live" ? ` · ${reh.mode === "clear" ? "text cleared" : reh.mode}` : ""} · not on the screens` : mainLabel;
  const gridSlides = rehearsal ? reh?.slides ?? [] : liveSlides;
  const gridItem = rehearsal ? reh?.item : live?.item;
  const latestHeard = speech?.recent[0];
  const heardChip = latestHeard && (
    <div
      className="heard"
      role="status"
      aria-live="polite"
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") e.preventDefault(), heardDismiss(latestHeard.reference);
      }}
    >
      <Icon name="mic" />
      <span className="heard__ref">
        Heard <strong>{latestHeard.reference}</strong>
        <small>“{latestHeard.heard}”</small>
      </span>
      <button className="wp-btn wp-btn--quiet" onClick={() => heardPreview(latestHeard.reference)}>
        Preview
      </button>
      <button className="wp-btn wp-btn--primary" onClick={() => heardLive(latestHeard.reference)} title="Enter on this button sends it live; Esc dismisses">
        Go live
      </button>
      {speech!.recent.length > 1 && (
        <select className="heard__recent" value="" onChange={(e) => e.target.value && heardPreview(e.target.value)} aria-label="Earlier suggestions">
          <option value="">Earlier ({speech!.recent.length - 1})</option>
          {speech!.recent.slice(1).map((r) => (
            <option key={r.reference + r.at} value={r.reference}>
              {r.reference}
            </option>
          ))}
        </select>
      )}
      <button className="wp-icon-btn" aria-label={`Dismiss ${latestHeard.reference}`} onClick={() => heardDismiss(latestHeard.reference)}>
        <Icon name="close" />
      </button>
    </div>
  );

  const livePane = (
    <section className="wp-pane wp-pane--work ew-pane" aria-label="Live" data-region="live">
      <header className="wp-pane__head">
        <h2 className={`wp-live-state wp-live-state--${stateClass}`}>
          <span className="wp-dot" aria-hidden="true" />
          {liveLabel}
        </h2>
        <span className="wp-pane__sub" title={liveTitle}>
          {liveTitle}
        </span>
        {/* the song's languages, always in sight (amber when not both) */}
        {live?.item?.kind === "song" && !rehearsal && liveLangs.length > 1 && (
          <div className={`lang-switch ${(state?.lang_mode ?? "both") !== "both" ? "is-limited" : ""}`} role="group" aria-label="Languages on the screens">
            {([
              ["primary", langName(liveLangs[0] ?? "en")],
              ["both", "Both"],
              ["second", liveLangs[1] ? langName(liveLangs[1]) : "Second"],
            ] as const).map(([m, l]) => (
              <button key={m} className={(state?.lang_mode ?? "both") === m ? "is-on" : ""} aria-pressed={(state?.lang_mode ?? "both") === m} onClick={() => void run(api.setLayout(undefined, m as LangMode))}>
                {l}
              </button>
            ))}
          </div>
        )}
        <ViewToggle value={liveView} onChange={setLiveView} />
      </header>
      {heardChip}
      {speech?.listening && <div className="heard__caption" title="What the speech engine is hearing">{speech.caption ? `Hearing: ${speech.caption}` : `Listening (${speech.listening})…`}</div>}
      {main.condition === "none" && !mainNdi && !rehearsal && (
        // a slim amber notice, not a block pushing the slides down
        <div className="live-notice" role="status">
          <span>No screen is connected: the slides show here only.</span>
          <button className="wp-btn wp-btn--quiet" onClick={() => setLayer("outputs")}>
            Set up a screen
          </button>
        </div>
      )}
      {/* timelines: a song in time with a backing track */}
      {!rehearsal && <TimelineBar item={live?.item} slideIdx={state?.slide_idx ?? 0} slides={liveSlides} onShow={show} />}
      {/* a hymn's tune, once per verse, the slides in step */}
      {!rehearsal && live?.item?.kind === "song" && <TuneBar songId={live.item.song_id} slides={liveSlides} slideIdx={state?.slide_idx ?? 0} onShow={show} />}
      {/* ProPresenter's group buttons: straight to the chorus, the bridge… */}
      {!rehearsal && live?.item?.kind === "song" && <GroupButtons slides={gridSlides} current={state?.slide_idx ?? -1} onGo={(i) => show(i)} />}
      <div className="ew-list">
        {liveView === "list" ? (
          <SlideList
            slides={gridSlides}
            current={rehearsal ? reh?.slide_idx ?? -1 : state?.slide_idx ?? -1}
            onPick={(i) => show(i)}
            live
            confirmed={rehearsal ? reh?.mode === "live" : main.condition === "confirmed" && state?.mode === "live"}
            empty={rehearsal ? "Rehearsal: stage an item in Preview, then Go live. Nothing reaches the screens." : "Nothing live yet. Double-click a slide in Preview or an item in the schedule, or press Go live (F5)."}
          />
        ) : (
          <SlideGrid
        slides={gridSlides}
        kind={gridItem?.kind}
        media={rehearsal ? reh?.media : live?.media}
        themeId={themeIdOf(gridItem) ?? (rehearsal ? previewRule : live?.rule_theme_id ?? null)}
        current={rehearsal ? reh?.slide_idx ?? -1 : state?.slide_idx ?? -1}
        onPick={(i) => show(i)}
        empty={rehearsal ? "Rehearsal: stage an item in Preview, then Go live. Nothing reaches the screens." : "Nothing live yet. Stage an item in Preview, then Go live."}
        live
        confirmed={rehearsal ? reh?.mode === "live" : main.condition === "confirmed" && state?.mode === "live"}
      />
        )}
      </div>
      <div className="ew-out">
      <label className="ew-out__which" title="Which output the Live picture shows (as EasyWorship's Live Output menu)">
        <select value={screens.some((x) => x.role === monitorChoice) ? monitorChoice : "main"} onChange={(e) => setMonitorRole(e.target.value)} aria-label="Live picture shows">
          {(screens.length ? screens : [{ role: "main", name: "Main" }]).map((x) => (
            <option key={x.role} value={x.role}>
              {x.name}
            </option>
          ))}
        </select>
      </label>
      <div ref={liveSurface} className="wp-live-surface ew-out__mon" tabIndex={0} aria-label="Live. Space or right arrow: next slide. Left arrow: previous. Escape: blank. L: logo. C: clear.">
          <div className="wp-monitor wp-monitor--flush">
            {monitorRole === "foldback" ? (
              <StageView b={conf as Broadcast | null} lost={false} />
            ) : monitorRole === "alternate" ? (
              // the lower third: song and scripture words only, see-through elsewhere
              conf?.item && (conf.item.kind === "song" || conf.item.kind === "scripture") && confState?.mode !== "blank" && confState?.mode !== "logo" ? (
                <SlideView
                  slide={confSlide}
                  allSlides={confSlides}
                  look={{ lowerThird: true }}
                  kind={conf.item.kind}
                  caption={conf.caption}
                  position={{ first: (confState?.slide_idx ?? 0) === 0, last: (confState?.slide_idx ?? 0) >= confSlides.length - 1 }}
                  adjust={{ scale: confState?.text_scale ?? 1, shift: confState?.text_shift ?? 0 }}
                  hideText={confState?.mode === "clear"}
                />
              ) : (
                <div className="tv tv--keyed" aria-label="Alternate output: see-through" />
              )
            ) : (
              <SlideView
              slide={confSlide}
              allSlides={confSlides}
              position={{ first: (confState?.slide_idx ?? 0) === 0, last: (confState?.slide_idx ?? 0) >= confSlides.length - 1 }}
              adjust={{ scale: confState?.text_scale ?? 1, shift: confState?.text_shift ?? 0 }}
              kind={conf?.item?.kind}
              caption={conf?.caption}
              media={conf?.media}
              themeId={(conf?.item && themeIdOf(conf.item)) ?? conf?.rule_theme_id ?? null}
              playing={confState?.media_playing}
              restart={confState?.media_restart}
              blank={!conf?.item || confState?.mode === "blank" || confState?.mode === "logo"}
              hideText={confState?.mode === "clear"}
            />
            )}
            {!rehearsal && (main.condition === "unconfirmed" || main.condition === "disconnected" || main.condition === "sending") && (
              <div className={`wp-monitor__flag ${main.condition === "sending" ? "" : "is-warn"}`}>
                {main.condition === "sending"
                  ? "Sending…"
                  : `${main.condition === "disconnected" ? "Disconnected" : "Not confirmed"}. Showing the last confirmed picture${main.confirmedAt ? " from " + new Date(main.confirmedAt).toLocaleTimeString() : ""}.`}
              </div>
            )}
          </div>
        </div>
        <div className="ew-out__info">
          <span className="ew-out__label">Live output</span>
          <strong className="ew-out__title">{conf?.item ? itemTitle(conf.item) : "Nothing on the screens"}</strong>
          {conf?.item && confSlides.length > 0 && (
            <span className="wp-num">
              Slide {(confState?.slide_idx ?? 0) + 1} of {confSlides.length}
            </span>
          )}
          <div className="ew-out__nav">
            <button className="wp-icon-btn" aria-label="Previous slide" onClick={() => step(-1)} disabled={!gridSlides.length}>
              ‹
            </button>
            <button className="wp-icon-btn" aria-label="Next slide" onClick={() => step(1)} disabled={!gridSlides.length}>
              ›
            </button>
          </div>
        </div>
      </div>
      <div className="wp-live-tools ew-tools">
        {live?.media && live.media.kind !== "image" && (
          <>
            <button className="wp-btn" onClick={() => void run(api.mediaControl(!state?.media_playing, false))}>
              {state?.media_playing ? "Pause video" : "Play video"}
            </button>
            <button className="wp-btn wp-btn--quiet" onClick={() => void run(api.mediaControl(null, true))}>
              Restart
            </button>
          </>
        )}
        {!rehearsal && live?.item && (
          <button className="wp-btn wp-btn--quiet" onClick={() => void run(api.undoGoLive())} title="Go back to the item that was live before (Ctrl+Z on the Live pane)">
            Back to previous item
          </button>
        )}
        {!rehearsal && live?.item?.kind === "song" && (
          <button className="wp-btn wp-btn--quiet" onClick={() => openTab({ id: "song", songId: (live.item as Extract<ProgramItem, { kind: "song" }>).song_id })}>
            Edit words
          </button>
        )}
        <span className="wp-spacer" />
        {!rehearsal && (live?.item?.kind === "song" || live?.item?.kind === "scripture") && (
          <span className="wp-transpose" title="For this item only: the words larger or smaller, higher or lower on every screen. Resets on the next item.">
            Words
            <button className="wp-icon-btn" aria-label="Words smaller" onClick={() => run(api.setTextAdjust((state?.text_scale ?? 1) - 0.1, state?.text_shift ?? 0))}>
              A−
            </button>
            <button className="wp-icon-btn" aria-label="Words larger" onClick={() => run(api.setTextAdjust((state?.text_scale ?? 1) + 0.1, state?.text_shift ?? 0))}>
              A+
            </button>
            <button className="wp-icon-btn" aria-label="Words up" onClick={() => run(api.setTextAdjust(state?.text_scale ?? 1, (state?.text_shift ?? 0) - 2))}>
              ↑
            </button>
            <button className="wp-icon-btn" aria-label="Words down" onClick={() => run(api.setTextAdjust(state?.text_scale ?? 1, (state?.text_shift ?? 0) + 2))}>
              ↓
            </button>
            {((state?.text_scale ?? 1) !== 1 || (state?.text_shift ?? 0) !== 0) && (
              <button className="wp-icon-btn" aria-label="Words as the theme has them" title={`${Math.round((state?.text_scale ?? 1) * 100)}%, ${state?.text_shift ?? 0 > 0 ? "+" : ""}${state?.text_shift ?? 0}`} onClick={() => run(api.setTextAdjust(1, 0))}>
                ⟲
              </button>
            )}
          </span>
        )}
        {!rehearsal && live?.item?.kind === "song" && liveSlides.some((x) => x.chords?.length) && (
          <span className="wp-transpose" title="Moves the chords on the stage display for the musicians; the congregation sees no change">
            Chords
            <button className="wp-icon-btn" aria-label="Chords down a semitone" onClick={() => run(api.setTranspose((state?.transpose ?? 0) - 1))}>
              −
            </button>
            <span className="wp-num">{state?.transpose ? (state.transpose > 0 ? `+${state.transpose}` : state.transpose) : "0"}</span>
            <button className="wp-icon-btn" aria-label="Chords up a semitone" onClick={() => run(api.setTranspose((state?.transpose ?? 0) + 1))}>
              +
            </button>
          </span>
        )}
        {!rehearsal && live?.item?.kind === "song" && (
          <label className="wp-check wp-check--inline" title="One line at a time for call-and-response singing; the stage display shows the next line large">
            <input type="checkbox" checked={!!state?.lining_out} onChange={(e) => void run(api.setLiningOut(e.target.checked))} /> Lining out
          </label>
        )}
        {live?.item?.kind === "song" && !state?.lining_out && (
          <label className="wp-inline-select">
            <span>Lines</span>
            <select value={state?.lines_per_slide ?? 2} onChange={(e) => void run(api.setLayout(Number(e.target.value)))}>
              {[1, 2, 3, 4].map((n) => (
                <option key={n}>{n}</option>
              ))}
            </select>
          </label>
        )}

      </div>
      {!rehearsal && state?.auto && live?.item?.kind === "announcement" && <AutoBar state={state} onPause={(p) => void run(api.setAutoPaused(p))} />}
      <form
        className="wp-stage-note"
        onSubmit={(e) => {
          e.preventDefault();
          void run(api.setStageMessage(stageNote));
        }}
      >
        <input value={stageNote} onChange={(e) => setStageNote(e.target.value)} placeholder="Message to the stage display only" aria-label="Message to the stage display" />
        <button className="wp-btn wp-btn--quiet" type="submit" disabled={!stageNote.trim()}>
          Send
        </button>
        {state?.stage_message && (
          <button className="wp-btn wp-btn--quiet" type="button" onClick={() => (setStageNote(""), void run(api.setStageMessage(null)))}>
            Clear note
          </button>
        )}
      </form>
    </section>
  );

  const editorTab = tabs.find((t) => t.id === activeTab);
  // the song being edited, if it is in the open service (prefer the selected row)
  const editingSongId = editorTab?.id === "song" ? editorTab.songId : null;
  const songInService = (() => {
    if (editingSongId == null) return null;
    const sel = selected != null ? schedule[selected] : null;
    if (sel?.kind === "song" && sel.song_id === editingSongId) return { index: selected!, item: sel };
    const index = schedule.findIndex((it) => it.kind === "song" && it.song_id === editingSongId);
    return index >= 0 ? { index, item: schedule[index] as Extract<ProgramItem, { kind: "song" }> } : null;
  })();
  return (
    <div className={`wp-control ${prefs.lowSpecUi ? "is-lowspec" : ""}`} ref={shell}>
      <nav className="wp-wstabs" aria-label="Workspace">
        {tabs.map((t) => (
          <div key={t.id} className={`wp-wstab ${activeTab === t.id ? "is-active" : ""}`}>
            <button onClick={() => setActiveTab(t.id)} aria-current={activeTab === t.id}>
              {t.id === "service" ? "Service" : t.id === "themes" ? "Themes" : t.id === "settings" ? "Settings" : t.id === "ann" ? "Announcements" : t.songId == null ? "New song" : "Song editor"}
            </button>
            {t.id !== "service" && (
              <button className="wp-wstab__x" aria-label="Close tab" onClick={() => closeTab(t.id)}>
                <Icon name="close" />
              </button>
            )}
          </div>
        ))}
        <span className="wp-spacer" />
        <span className="wp-wstabs__app">
          <img src="/brand/icon-64.png" alt="" width={16} height={16} />
          Worship Presenter
        </span>
        <span className="wp-spacer" />
        <button
          className="wp-theme-toggle"
          onClick={() => setUiTheme(document.documentElement.dataset.theme === "light" ? "dark" : "light")}
          title={document.documentElement.dataset.theme === "light" ? "Switch to the dark theme" : "Switch to the light theme"}
          aria-label="Switch between light and dark"
        >
          <Icon name={uiTheme === "light" || (uiTheme === "system" && document.documentElement.dataset.theme === "light") ? "moon" : "sun"} />
        </button>
        <button className="wp-btn wp-btn--quiet" onClick={() => (setSettingsPage(undefined), openTab({ id: "settings" }))} title="Settings: appearance, brand, outputs, NDI, import, backups, profiles… (Ctrl+K searches everything)">
          <Icon name="settings" /> Settings
        </button>
      </nav>

      {activeTab === "service" ? (
        <header ref={barRef} className={`wp-bar ${twoRows ? "wp-bar--two" : ""}`}>
          <div className="wp-bar__where">
            <button className="wp-crumb" onClick={() => setLayer("profiles")} title="Profiles">
              {profile.name || "Profile"}
            </button>
            <span className="wp-crumb__sep" aria-hidden="true">
              /
            </span>
            <button className="wp-crumb wp-crumb--strong" onClick={() => setLayer("service")} title="Open or start a service order">
              {service?.name ?? "Unsaved service"}
            </button>
          </div>
          <label className="wp-search wp-search--quick">
            <Icon name="book" />
            <input
              ref={quickInput}
              value={quick}
              placeholder="555, MHB 1 or Yohane 3:16"
              aria-label="Hymn number or scripture. Enter puts it in Preview."
              onChange={(e) => setQuick(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && quick.trim()) e.preventDefault(), void quickStage();
                if (e.key === "Escape") e.preventDefault(), (e.target as HTMLInputElement).blur();
              }}
            />
          </label>
          <span className="wp-spacer" />
          <button className={`wp-btn wp-btn--quiet wp-listen ${speech?.listening ? "is-on" : ""}`} onClick={toggleListening} title={speech?.listening ? "Stop listening for Bible references" : "Listen to the preacher's microphone for Bible references (experimental)"} aria-pressed={!!speech?.listening}>
            <Icon name="mic" /> {speech?.starting ? "Starting…" : speech?.listening ? "Listening" : "Listen"}
          </button>
          <button className={`wp-btn wp-btn--quiet ${rec?.running ? "rec-on" : ""}`} onClick={() => setLayer(layer === "record" ? null : "record")} title="Record the service, or go live on YouTube, Facebook or StreamRelay">
            {rec?.running ? <span className="rec__dot" aria-hidden="true" /> : <Icon name="video" />} {rec?.running ? `${Math.floor(rec.seconds / 60)}:${String(rec.seconds % 60).padStart(2, "0")}` : "Record"}
          </button>
          {/* the tools used less often, in one menu; any that is on shows as a chip */}
          {rehearsal && <span className="tool-chip is-on">Rehearsal</span>}
          {state?.camera && <span className="tool-chip is-on">Camera on</span>}
          {state?.subtitles && <span className="tool-chip is-on">Subtitles</span>}
          {music?.status.playing && (
            <button className="tool-chip is-on" onClick={() => setLayer("music")}>
              <Icon name="music" /> {music.status.paused ? "Paused" : music.status.title ?? "Music"}
            </button>
          )}
          {preService && <span className="tool-chip is-on">Pre-service · {preService.startsAt}</span>}
          <div className="tools-menu">
            <button className={`wp-btn wp-btn--quiet ${toolsOpen ? "is-on" : ""}`} aria-expanded={toolsOpen} aria-haspopup="true" onClick={() => setToolsOpen((x) => !x)} title="Rehearsal, transcript, camera, subtitles, music, pre-service">
              Tools ▾
            </button>
            {toolsOpen && (
              <>
                <div className="tools-menu__away" onMouseDown={() => setToolsOpen(false)} />
                <div className="tools-menu__pop" role="menu" onClick={(e) => (e.target as HTMLElement).closest("button") && setToolsOpen(false)}>
              <label className={`wp-switch ${rehearsal ? "is-on" : ""}`} title="Practise the service without sending anything to the screens">
                <input type="checkbox" checked={rehearsal} onChange={(e) => (setRehearsal(e.target.checked), setReh(null))} /> Rehearsal
              </label>

              <button className={`wp-btn wp-btn--quiet ${transcriptOpen ? "is-on" : ""} ${speech?.warning ? "is-warn" : ""}`} onClick={() => setTranscriptOpen((x) => !x)} title="The live transcript, the microphone level and the scriptures detected">
                Transcript{speech?.warning ? " ⚠" : ""}
              </button>
              <button className={`wp-btn wp-btn--quiet wp-cam ${state?.camera ? "is-on" : ""}`} onClick={() => setLayer(layer === "camera" ? null : "camera")} title="A camera on the main screen">
                <Icon name="video" /> {state?.camera ? "Camera on" : "Camera"}
              </button>
              <select
                className={`wp-subs ${state?.subtitles ? "is-on" : ""}`}
                value={state?.subtitles ?? ""}
                title="Live sermon subtitles (experimental): what the preacher says, as captions"
                aria-label="Subtitles"
                onChange={async (e) => {
                  const t = (e.target.value || null) as "stream" | "screens" | "both" | null;
                  await run(api.setSubtitles(t));
                  // captions need the whole sermon, not only Bible references
                  if (t && (!speech?.listening || speechCfg.refsOnly)) {
                    try {
                      await api.speechStart(speechCfg.engine, speechCfg.mic || null, false);
                      say("Subtitles on. Listening to everything the preacher says.");
                    } catch (err) {
                      say(String(err instanceof Error ? err.message : err), true);
                    }
                  }
                }}
              >
                <option value="">Subtitles off</option>
                <option value="stream">Subtitles: stream</option>
                <option value="screens">Subtitles: screens</option>
                <option value="both">Subtitles: both</option>
              </select>
              <button className={`wp-btn wp-btn--quiet wp-music-chip ${music?.status.playing ? "is-on" : ""}`} onClick={() => setLayer(layer === "music" ? null : "music")} title={music?.status.title ?? "Background music"}>
                <Icon name="music" /> {music?.status.playing ? (music.status.paused ? "Paused" : music.status.ducked ? "Music (quiet)" : music.status.title ?? "Music") : "Music"}
              </button>

              <button className={`wp-btn wp-btn--quiet ${preService ? "is-on" : ""}`} onClick={() => setLayer(layer === "preservice" ? null : "preservice")} title="Countdown, announcements and music before the service; the first item goes live on time">
                <Icon name="timer" /> {preService ? `Pre-service · ${preService.startsAt}` : "Pre-service"}
              </button>
                </div>
              </>
            )}
          </div>
          <TimerChip timer={state?.timer ?? null} onClick={() => setLayer(layer === "timer" ? null : "timer")} />
          <button className={`wp-btn wp-btn--quiet ${state?.alert ? "is-on" : ""}`} onClick={() => setLayer(layer === "alert" ? null : "alert")}>
            <Icon name="alert" /> {state?.alert ? "Alert on" : "Alert"}
          </button>
          <ServiceActions state={state} latched={latched} onMode={setMode} onResume={resume} />
          {/* the Take button (as a vision mixer's): Preview to the screens; always in reach */}
              <button
                className="wp-btn wp-btn--primary take"
                onClick={() => void goLive()}
                disabled={!canGoLive}
                title={latched ? "Show this item on the screens, ending Blank / Logo (F5)" : main.condition === "none" ? "No projector connected: the slide shows in Live only (F5)" : "Send the staged slide to the screens (F5)"}
              >
                <Icon name="take" />
                <span className="take__label">Go live</span>
                <kbd>F5</kbd>
              </button>

        </header>
      ) : (
        <header className="wp-bar wp-bar--strip">
          <span className={`wp-live-state wp-live-state--${stateClass}`}>
            <span className="wp-dot" aria-hidden="true" />
            {mainLabel}
          </span>
          <span className="wp-pane__sub">{conf?.item ? itemTitle(conf.item) : ""}</span>
          <span className="wp-spacer" />
          <button className={`wp-btn wp-mode ${state?.mode === "blank" ? "is-on is-on--blank" : ""}`} aria-pressed={state?.mode === "blank"} onClick={() => toggleMode("blank")} title={state?.mode === "blank" ? "Blank is on: press again to bring the words back" : "Black screens"}>
            {state?.mode === "blank" && <span className="wp-mode__dot" aria-hidden="true" />}
            <Icon name="blank" /> Blank{state?.mode === "blank" && <span className="wp-mode__on">ON</span>}
          </button>
          <button className="wp-btn" onClick={() => setActiveTab("service")}>
            Return to service
          </button>
        </header>
      )}

      {update && (
        <div className="wp-banner" role="status">
          <span>Worship Presenter {update.version} is available.{update.notes ? ` ${update.notes}` : ""}</span>
          <button className="wp-btn wp-btn--primary" onClick={() => void api.updateInstall().catch((e) => say(String(e instanceof Error ? e.message : e), true))}>
            Install and restart
          </button>
          <button className="wp-btn wp-btn--quiet" onClick={() => setUpdate(null)}>
            Later
          </button>
        </div>
      )}
      {rehearsal && (
        <div className="wp-banner wp-banner--warn" role="status">
          <span>Rehearsal mode. Go live, Blank and the Live pane drive a simulator; nothing reaches the screens.</span>
          <button className="wp-btn" onClick={() => (setRehearsal(false), setReh(null))}>
            End rehearsal
          </button>
        </div>
      )}
      {held.length > 0 && (
        <div className="wp-banner wp-banner--warn" role="alert">
          <span>{held.map((r) => ROLE_NAMES[r] ?? r).join(" and ")} came back after being lost and is showing black.</span>
          {held.map((r) => (
            <button key={r} className="wp-btn" onClick={() => void run(api.releaseOutput(r))}>
              Resume {ROLE_NAMES[r] ?? r}
            </button>
          ))}
        </div>
      )}
      {notice && (
        <div className={`wp-banner ${notice.bad ? "wp-banner--bad" : ""}`} role={notice.bad ? "alert" : "status"}>
          <span>{notice.text}</span>
          {notice.action && (
            <button className="wp-btn" onClick={() => (notice.action!.run(), setNotice(null))}>
              {notice.action.label}
            </button>
          )}
          <button className="wp-icon-btn" aria-label="Dismiss" onClick={() => setNotice(null)}>
            <Icon name="close" />
          </button>
        </div>
      )}

      <div className="wp-work">
        {activeTab === "service" ? (
          <div className="ew" ref={ewRef}>
            <div className="ew-top" style={{ height: `${ew.top}%` }}>
              {wide && (
                <div className="wp-cell ew-cell" style={{ width: ew.schedule }}>
                  {schedulePane}
                  <Splitter label="Resize schedule" onDrag={(dx) => saveEw({ ...ew, schedule: Math.max(170, Math.min(420, ew.schedule + dx)) })} />
                </div>
              )}
              <div className="wp-cell ew-cell" style={{ flex: ew.split }}>
                {previewPane}
                <Splitter label="Resize preview and live" onDrag={(dx) => saveEw({ ...ew, split: Math.max(0.25, Math.min(0.75, ew.split + dx / Math.max(1, width - (wide ? ew.schedule : 0)))) })} />
              </div>
              <div className="wp-cell ew-cell" style={{ flex: 1 - ew.split }}>
                {livePane}
              </div>
            </div>
            <div
              className="ew-hsplit"
              role="separator"
              aria-orientation="horizontal"
              aria-label="Resize library"
              tabIndex={0}
              onPointerDown={(e) => {
                e.preventDefault();
                const h = ewRef.current?.clientHeight ?? 800;
                let y = e.clientY;
                let top = ew.top;
                const move = (m: PointerEvent) => {
                  top = Math.max(30, Math.min(80, top + ((m.clientY - y) / h) * 100));
                  y = m.clientY;
                  setEw((x) => ({ ...x, top }));
                };
                const up = () => (window.removeEventListener("pointermove", move), window.removeEventListener("pointerup", up), saveEw({ ...ew, top }));
                window.addEventListener("pointermove", move);
                window.addEventListener("pointerup", up);
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowUp") e.preventDefault(), saveEw({ ...ew, top: Math.max(30, ew.top - 2) });
                if (e.key === "ArrowDown") e.preventDefault(), saveEw({ ...ew, top: Math.min(80, ew.top + 2) });
              }}
            />
            <div className="ew-bottom">{libraryDock}</div>
          </div>
        ) : editorTab?.id === "themes" ? (
          <ThemeEditor request={themeRequest} onClose={() => closeTab("themes")} />
        ) : editorTab?.id === "ann" ? (
          <AnnouncementEditor
            key={`${editorTab.deckId}-${editorTab.template?.title ?? ""}`}
            deckId={editorTab.deckId}
            template={editorTab.template}
            onClose={() => closeTab("ann")}
            onSaved={(id, title) => {
              setLibraryVersion((v) => v + 1);
              if (id < 0) return;
              setSchedule((sc) => sc.map((it) => (it.kind === "announcement" && it.presentation_id === id ? { ...it, title } : it)));
              if (preview?.item.kind === "announcement" && preview.item.presentation_id === id) void load({ ...preview.item, title });
              if (live?.item?.kind === "announcement" && live.item.presentation_id === id) say("Saved. The announcements on the screens keep their old slides until you send them live again.");
            }}
          />
        ) : editorTab?.id === "settings" ? (
          <Settings
            initial={settingsPage}
            uiTheme={uiTheme}
            setUiTheme={setUiTheme}
            views={view}
            mainLive={!!live?.item && state?.mode === "live"}
            onOpenOutputs={() => setLayer("outputs")}
            onOpen={(what) => setLayer(what)}
            onResetPanes={() => panes.reset()}
          />
        ) : editorTab?.id === "song" ? (
          <SongEditor
            key={String(editorTab.songId)}
            songId={editorTab.songId}
            onClose={() => closeTab("song")}
            serviceItem={songInService ? (songInService.item as { lyrics?: { lang: string; body: string }[] | null }) : null}
            onApply={
              editorTab.songId != null
                ? (lyrics) => {
                    const id = editorTab.songId!;
                    const title = songInService?.item.title ?? preview?.caption ?? "";
                    if (songInService) setSchedule((sc) => sc.map((it, i) => (i === songInService.index && it.kind === "song" ? { ...it, lyrics } : it)));
                    else setSchedule((sc) => [...sc, { kind: "song", song_id: id, title, lyrics }]);
                    if (preview?.item.kind === "song" && preview.item.song_id === id) void load({ ...preview.item, lyrics });
                    say(lyrics ? "These words are used in this service only. The library is unchanged." : "This service uses the library words again.");
                  }
                : undefined
            }
            onSaved={(id, title, isLive) => {
              setLibraryVersion((v) => v + 1);
              setSchedule((sc) => sc.map((it) => (it.kind === "song" && it.song_id === id ? { ...it, title } : it)));
              if (preview?.item.kind === "song" && preview.item.song_id === id && !preview.item.lyrics) void load({ ...preview.item, title });
              if (isLive && !rehearsal)
                setNotice({ text: "Saved to the library. This song is on the screens and still shows the old words.", action: { label: "Update the screens", run: () => void run(api.refreshLiveItem(title)) } });
            }}
            onDeleted={(id) => {
              setLibraryVersion((v) => v + 1);
              setSchedule((sc) => sc.filter((it) => !(it.kind === "song" && it.song_id === id)));
              if (preview?.item.kind === "song" && preview.item.song_id === id) setPreview(null);
            }}
          />
        ) : null}
      </div>

      <footer className="wp-status">
        {["main", ...roles.filter((r) => r !== "main")].map((r) => {
          const v: OutputView = view(r);
          return (
            <button key={r} className={`wp-status__out wp-status__out--${v.condition}`} onClick={() => setLayer("outputs")} title="Output details">
              {outputLabel(v, state?.mode)}
            </button>
          );
        })}
        {conf?.item && confSlides.length > 0 && (
          <span className="wp-num">
            Slide {(confState?.slide_idx ?? 0) + 1} of {confSlides.length}
          </span>
        )}
        <span className="wp-spacer" />
        {saved && <span className={saved === "failed" ? "wp-status__bad" : ""}>{saved === "saving" ? "Saving…" : saved === "saved" ? "Service saved" : "Service not saved"}</span>}
        <button className="wp-status__out" onClick={() => setLayer("backups")}>
          Backup: {lastBackup ?? "none yet"}
        </button>
        {!isDesktop && <span>Browser preview</span>}
        <Clock />
      </footer>

      {layer === "palette" && <Palette commands={commands} onClose={() => setLayer(null)} />}
      {recovery && service && (recovery.schedule_id == null || recovery.schedule_id === service.id) && schedule[recovery.item_idx] && (
        <div className="recovery" role="alert">
          <strong>Worship Presenter closed unexpectedly</strong>
          <span>
            Pick up where you were: {itemTitle(schedule[recovery.item_idx])}, slide {recovery.slide_idx + 1}.
          </span>
          <button
            className="wp-btn wp-btn--primary"
            onClick={() => {
              const it = schedule[recovery.item_idx];
              void run(api.goLive(it, recovery.slide_idx)).then(() => say(`Back on the screens: ${itemTitle(it)}.`));
              setRecovery(null);
              void api.recoveryDismiss();
            }}
          >
            Resume
          </button>
          <button className="wp-btn wp-btn--quiet" onClick={() => (setRecovery(null), void api.recoveryDismiss())}>
            Dismiss
          </button>
        </div>
      )}
      {layer === "lectionary" && <LectionaryPanel serviceDate={service?.service_date ?? null} onAdd={(items) => setSchedule((sc) => [...sc, ...items])} onClose={() => setLayer(null)} />}
      {exportIds && <SongExportDialog ids={exportIds} onClose={() => setExportIds(null)} />}
      {(layer === "package-export" || layer === "package-import") && <PackageDialog mode={layer === "package-export" ? "export" : "import"} onClose={() => setLayer(null)} />}
      {layer === "props" && <PropsPanel showing={state?.props ?? []} onClose={() => setLayer(null)} />}
      {layer === "macros" && <MacrosPanel onRun={(m) => void runMacro(m)} onClose={() => setLayer(null)} />}
      {slideMenu && preview && (
        <ContextMenu x={slideMenu.x} y={slideMenu.y} onClose={() => setSlideMenu(null)}>
          <div className="wp-note" style={{ padding: "4px 10px" }}>
            When this slide goes live, run:
          </div>
          {[{ id: null, name: "Nothing" } as { id: number | null; name: string }, ...macroList].map((m) => {
            const k = slideKey(preview.item, preview.slides, slideMenu.i);
            const on = k && (previewActions.get(k[2]) ?? null) === m.id;
            return (
              <button
                key={m.id ?? "none"}
                role="menuitem"
                onClick={() => {
                  setSlideMenu(null);
                  if (!k) return;
                  void api.slideActionSet(k[0], k[1], k[2], m.id).then(() =>
                    api.slideActions(k[0], k[1]).then((l) => {
                      setPreviewActions(new Map(l));
                      if (liveItemKey === JSON.stringify(k.slice(0, 2))) setLiveActions(new Map(l));
                    }),
                  );
                }}
              >
                {on ? "✓ " : ""}
                {m.name}
              </button>
            );
          })}
        </ContextMenu>
      )}
      {layer === "outputs" && (
        <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && setLayer(null)}>
          <div className="wp-dialog wp-dialog--wide" role="dialog" aria-modal="true" aria-label="Options: outputs">
            <header className="wp-dialog__head">
              <h2>Options: outputs</h2>
              <button className="wp-btn wp-btn--quiet" onClick={() => setLayer(null)} autoFocus>
                Done
              </button>
            </header>
            <OutputOptions views={view} mainLive={!!live?.item && state?.mode === "live"} />
          </div>
        </div>
      )}
      {layer === "import" && <ImportDialog onClose={() => (setLayer(null), setLibraryVersion((v) => v + 1))} />}
      {layer === "backups" && <BackupsDialog onClose={() => (setLayer(null), reopenLast())} onRestored={afterRestore} />}
      {layer === "profiles" && <ProfilesDialog somethingLive={state?.mode === "live" && !!live?.item} onClose={() => (setLayer(null), api.currentProfile().then(setProfile).catch(() => {}))} />}
      {layer === "service" && <ServiceMenu current={service?.id ?? null} onOpen={openService} onClose={() => setLayer(null)} />}
      {layer === "timer" && <TimerPanel state={state} onClose={() => setLayer(null)} />}
      {transcriptOpen && (
        <TranscriptPanel
          speech={speech}
          onStart={() => void (speech?.listening ? undefined : toggleListening())}
          onStop={() => void api.speechStop()}
          onPreview={(r) => heardPreview(r)}
          onLive={(r) => heardLive(r)}
          onClose={() => setTranscriptOpen(false)}
        />
      )}
      {layer === "dashboard" && (
        <OutputsPanel
          onClose={() => setLayer(null)}
          onFix={(where) => {
            setLayer(null);
            if (where === "screens") void api.screensPower(true);
            else (setSettingsPage(where as Parameters<typeof setSettingsPage>[0]), openTab({ id: "settings" }));
          }}
        />
      )}
      {layer === "replace" && <FindReplace onClose={() => setLayer(null)} onDone={(n) => (setLayer(null), setLibraryVersion((v) => v + 1), say(`Changed ${n} song${n === 1 ? "" : "s"}. A backup was made first (Settings → Backups).`))} />}
      {layer === "lyrics" && (
        <LyricsFinder
          onClose={() => setLayer(null)}
          onAdded={(id, t) => (setLayer(null), setLibraryVersion((v) => v + 1), openTab({ id: "song", songId: id }), say(`“${t}” is in your songs: check the verses and chorus, then save.`))}
        />
      )}
      {layer === "record" && <RecordPanel status={rec} onClose={() => setLayer(null)} refresh={() => void api.streamStatus().then(setRec).catch(() => {})} />}
      {layer === "outline" && <OutlineDialog onClose={() => setLayer(null)} onMade={(t) => (setLayer(null), setLibraryVersion((v) => v + 1), say(`“${t}” is in Presentations.`))} />}
      {layer === "offering" && (
        <OfferingSlide onClose={() => setLayer(null)} onMade={(t) => (setLayer(null), setLibraryVersion((v) => v + 1), say(`“${t}” is in Presentations.`))} />
      )}
      {layer === "preservice" && (
        <PreServicePanel
          firstItem={schedule[0] ?? null}
          running={preService}
          onClose={() => setLayer(null)}
          onStop={() => (setPreService(null), void api.timerControl("stop").catch(() => {}), setLayer(null), say("Pre-service loop stopped."))}
          onStart={async (plan) => {
            try {
              if (plan.deckId) {
                // the announcements turn by themselves, round and round
                const deck = await api.getAnnouncement(plan.deckId);
                if (deck.seconds !== plan.seconds || !deck.looping) await api.saveAnnouncement({ ...deck, seconds: plan.seconds, looping: true });
                await api.goLive({ kind: "announcement", presentation_id: plan.deckId, title: deck.title }, 0);
              }
              await api.timerStart("Service starts in", "to_time", plan.startsAt, plan.countdown, true);
              if (plan.music) await api.musicPlay(null).catch(() => {});
              setPreService(plan);
              setLayer(null);
              say(`Pre-service loop running until ${plan.startsAt}.`);
            } catch (e) {
              say(String(e instanceof Error ? e.message : e), true);
            }
          }}
        />
      )}
      {layer === "camera" && <CameraPanel live={state?.camera ?? null} onClose={() => setLayer(null)} say={say} />}
      {layer === "music" && <MusicPanel m={music} onClose={() => setLayer(null)} />}
      {layer === "alert" && <AlertPanel alert={state?.alert} onClose={() => setLayer(null)} />}
    </div>
  );
}

// Resume is the lit switch pressed again (and Live → Resume in the menu bar)
