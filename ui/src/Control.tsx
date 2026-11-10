import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api, isDesktop, onLibraryRestored, onProfileChanged, onSettingsChanged, type LangMode, type LiveState, type MediaItem, type Mode, type ProgramItem, type Schedule, type Slide, type Timer } from "./api";
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
import { fmtLeft, timerLeft, useNow } from "./Cues";
import type { ThemeRow } from "./theme/types";
import { Palette, type PaletteCommand } from "./features/commands/Palette";
import { SchedulePane, SlideGrid, itemTitle, sameItem, themeIdOf } from "./features/service/panes";
import { AnnouncementEditor } from "./features/editors/AnnouncementEditor";
import { LibraryDock, type LibTab } from "./features/library/LibraryDock";
import { SlideList } from "./features/service/SlideList";
import { useMediaUrl } from "./theme/media";
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
const LANG_NAMES: Record<string, string> = { en: "English", fat: "Fante", tw: "Twi", gaa: "Ga", ee: "Ewe" };

function isTyping(el: EventTarget | null) {
  const t = el as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}

// ---------- UI theme: Dark, Light or Follow Windows ----------

type UiTheme = "dark" | "light" | "system";
function applyUiTheme(t: UiTheme) {
  const dark = t === "dark" || (t === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
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
  const [layer, setLayer] = useState<null | "palette" | "outputs" | "import" | "backups" | "profiles" | "service" | "timer" | "alert" | "music">(null);
  const music = useMusic();
  const [profile, setProfile] = useState({ slug: "default", name: "" });
  const [libraryVersion, setLibraryVersion] = useState(0);
  const [themeRows, setThemeRows] = useState<ThemeRow[]>([]);
  const [stageNote, setStageNote] = useState("");
  const [advanceAfterLive, setAdvanceAfterLive] = useState(false);
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
  const [slideView, setSlideViewRaw] = useState<"list" | "thumbs">(() => (localStorage.getItem("wp-slide-view") as "list" | "thumbs") || "list");
  const setSlideView = (v: "list" | "thumbs") => (setSlideViewRaw(v), localStorage.setItem("wp-slide-view", v));

  const state = live?.state;
  const liveSlides = live?.slides ?? [];
  const main = view("main");
  const mainLabel = outputLabel(main, state?.mode);
  const coreLatched = !!live?.item && !!state && state.mode !== "live";
  const latched = rehearsal ? !!reh && reh.mode !== "live" : coreLatched;
  const held = live?.held ?? [];
  const canGoLive = rehearsal ? !!preview && !latched : !!preview && main.condition !== "none" && main.condition !== "disconnected" && !latched;

  const say = (text: string, bad = false) => setNotice({ text, bad });
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

  useEffect(() => {
    const json = JSON.stringify(schedule);
    if (json === loadedItems.current) return;
    if (service == null && schedule.length === 0) return;
    const t = setTimeout(async () => {
      try {
        setSaved("saving");
        let id = service?.id;
        if (id == null) {
          const today = new Date().toISOString().slice(0, 10);
          const created = await api.newSchedule("Untitled service", today, false, null);
          const { items: _ignored, ...meta } = created;
          setService(meta);
          id = created.id;
        }
        await api.saveSchedule(id, schedule);
        loadedItems.current = json;
        setSaved("saved");
      } catch {
        setSaved("failed");
      }
    }, 400);
    return () => clearTimeout(t);
  }, [schedule, service]);

  useEffect(() => {
    api.listThemes().then(setThemeRows).catch(() => {});
  }, [live?.theme_rev, activeTab]);

  // ---------- staging (never public) ----------

  const load = useCallback(
    async (item: ProgramItem) => {
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
      } else {
        const r = await run(api.previewScripture(item.reference, item.versions));
        if (r) setPreview({ item, slides: r[0], caption: r[1] });
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
      if (latched) return say("Rehearsal is held. Choose Resume output first.", true);
      setReh({ item: preview.item, slides: preview.slides, caption: preview.caption, media: preview.media, slide_idx: slideAt, mode: "live" });
      setTimeout(() => liveSurface.current?.focus(), 0);
      return;
    }
    if (!canGoLive) {
      if (main.condition === "none") return say("There is no main display yet. Use Set up output.", true);
      if (latched) return say("Output is held. Choose Resume output first.", true);
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
      const next = Math.max(0, Math.min(schedule.length - 1, (selected ?? -1) + d));
      setSelected(next);
      await load(schedule[next]);
    },
    [schedule, selected, load],
  );

  // ---------- keyboard scopes (brief §6) ----------

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
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
      // The Live driving surface itself, not a button or list inside the Live pane.
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
        return void setMode(act === "blank" ? "blank" : act === "logo" ? "logo" : "clear");
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

  // Closing the window during a service asks first; cancelling leaves output unchanged.
  useEffect(() => {
    if (!isDesktop) return;
    let un = () => {};
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) =>
      getCurrentWindow()
        .onCloseRequested((ev) => {
          if (live?.item && state?.mode === "live" && !confirm("Something is on the screens. Close Worship Presenter and end output?")) ev.preventDefault();
        })
        .then((u) => (un = u)),
    );
    return () => un();
  }, [live?.item, state?.mode]);

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
      { id: "ann", group: "Library", label: "Announcements (offering, birthdays, welcome loop)", keywords: "notices slides momo tithe", run: () => (setActiveTab("service"), setLibTab("presentations"), setSourceTab("library")) },
      { id: "media", group: "Library", label: "Media library", run: () => (setActiveTab("service"), setLibTab("media"), setSourceTab("library")) },
      { id: "import", group: "Library", label: "Import songs or Bibles…", run: () => setLayer("import") },
      { id: "backups", group: "Library", label: "Backups…", run: () => setLayer("backups") },
      { id: "profiles", group: "Library", label: "Profiles…", run: () => setLayer("profiles") },
      { id: "service", group: "Service", label: "Open or start a service order…", run: () => setLayer("service") },
      { id: "timer", group: "Service", label: "Timer…", run: () => setLayer("timer") },
      { id: "alert", group: "Service", label: "Alert…", run: () => setLayer("alert") },
      { id: "music", group: "Service", label: "Background music…", keywords: "audio playlist songs instrumental", run: () => setLayer("music") },
      { id: "live", group: "Service", label: "Focus Live (keyboard drives the screens)", run: () => (setActiveTab("service"), setTimeout(() => liveSurface.current?.focus())) },
      { id: "outputs", group: "Outputs", label: "Output setup…", run: () => setLayer("outputs") },
      { id: "themes", group: "Design", label: "Themes", run: () => openTab({ id: "themes" }) },
      { id: "settings", group: "Appearance", label: "Settings", keywords: "preferences keyboard shortcuts low spec", run: () => openTab({ id: "settings" }) },
      { id: "rehearsal", group: "Service", label: "Rehearsal mode on or off", keywords: "practice simulate", run: () => setRehearsal((r) => (setReh(null), !r)) },
      { id: "ui-dark", group: "Appearance", label: "Appearance: Dark", keywords: "theme colour", run: () => setUiTheme("dark") },
      { id: "ui-light", group: "Appearance", label: "Appearance: Light", keywords: "theme colour", run: () => setUiTheme("light") },
      { id: "ui-system", group: "Appearance", label: "Appearance: Follow Windows", keywords: "theme colour system", run: () => setUiTheme("system") },
      { id: "reset-panes", group: "Appearance", label: "Reset pane sizes", run: () => panes.reset() },
    ],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [panes.reset],
  );

  // ---------- quick scripture (stages only) ----------

  const quickStage = async () => {
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
    <LibraryDock
      tab={libTab}
      setTab={setLibTab}
      version={libraryVersion}
      songSearch={songSearch}
      refInput={refInput}
      previewKind={preview?.item.kind ?? null}
      onPreview={(item) => void load(item)}
      onAdd={(item) => setSchedule((sc) => [...sc, item])}
      onEditSong={(id) => openTab({ id: "song", songId: id })}
      onNewSong={() => openTab({ id: "song", songId: null })}
      onMediaPreview={(m) => (setPreview({ item: { kind: "media", media_id: m.id, title: m.title }, slides: [MEDIA_SLIDE], caption: m.title, media: m }), setPreviewIdx(0))}
      onEditDeck={(id) => openTab({ id: "ann", deckId: id, template: null })}
      onNewDeck={(template) => openTab({ id: "ann", deckId: null, template })}
      onUseTheme={(id) => setLook(id)}
      onEditThemes={() => openTab({ id: "themes" })}
      extraTab={wide ? undefined : { label: "Schedule", node: schedulePane }}
    />
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
        <ViewToggle value={slideView} onChange={setSlideView} />
      </header>
      <div className="ew-list">
        {!preview ? (
          <div className="wp-empty">
            <p>Select an item to preview.</p>
          </div>
        ) : slideView === "list" ? (
          <SlideList slides={preview.slides} current={previewIdx} onPick={setPreviewIdx} onActivate={(i) => (setPreviewIdx(i), void goLive(i))} empty="This item has no slides." />
        ) : (
          <SlideGrid
            slides={preview.slides}
            kind={preview.item.kind}
            media={preview.media}
            themeId={themeIdOf(preview.item)}
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
            <SlideView slide={previewSlide} kind={preview?.item.kind} caption={preview?.caption} media={preview?.media} themeId={themeIdOf(preview?.item)} still />
          )}
        </div>
        <div className="ew-out__info">
          <span className="ew-out__label">Preview output</span>
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
        <ViewToggle value={slideView} onChange={setSlideView} />
      </header>
      {main.condition === "none" && !rehearsal && (
        <div className="wp-empty wp-empty--inline">
          <p>No main display</p>
          <button className="wp-btn" onClick={() => setLayer("outputs")}>
            Set up output
          </button>
        </div>
      )}
      <div className="ew-list">
        {slideView === "list" ? (
          <SlideList
            slides={gridSlides}
            current={latched ? -1 : rehearsal ? reh?.slide_idx ?? -1 : state?.slide_idx ?? -1}
            onPick={(i) => (latched ? say("Output is held. Choose Resume output first.", true) : show(i))}
            live
            confirmed={rehearsal ? reh?.mode === "live" : main.condition === "confirmed" && state?.mode === "live"}
            empty={rehearsal ? "Rehearsal: stage an item in Preview, then Go live. Nothing reaches the screens." : "Nothing live yet. Stage an item in Preview, then Go live."}
          />
        ) : (
          <SlideGrid
        slides={gridSlides}
        kind={gridItem?.kind}
        media={rehearsal ? reh?.media : live?.media}
        themeId={themeIdOf(gridItem)}
        current={latched ? -1 : rehearsal ? reh?.slide_idx ?? -1 : state?.slide_idx ?? -1}
        onPick={(i) => (latched ? say("Output is held. Choose Resume output first.", true) : show(i))}
        empty={rehearsal ? "Rehearsal: stage an item in Preview, then Go live. Nothing reaches the screens." : "Nothing live yet. Stage an item in Preview, then Go live."}
        live
        confirmed={rehearsal ? reh?.mode === "live" : main.condition === "confirmed" && state?.mode === "live"}
      />
        )}
      </div>
      <div className="ew-out">
      <div ref={liveSurface} className="wp-live-surface ew-out__mon" tabIndex={0} aria-label="Live. Space or right arrow: next slide. Left arrow: previous. Escape: blank. L: logo. C: clear.">
          <div className="wp-monitor wp-monitor--flush">
            <SlideView
              slide={confSlide}
              kind={conf?.item?.kind}
              caption={conf?.caption}
              media={conf?.media}
              themeId={conf?.item && themeIdOf(conf.item)}
              playing={confState?.media_playing}
              restart={confState?.media_restart}
              blank={!conf?.item || confState?.mode === "blank" || confState?.mode === "logo"}
              hideText={confState?.mode === "clear"}
            />
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
            <button className="wp-icon-btn" aria-label="Previous slide" onClick={() => step(-1)} disabled={!gridSlides.length || latched}>
              ‹
            </button>
            <button className="wp-icon-btn" aria-label="Next slide" onClick={() => step(1)} disabled={!gridSlides.length || latched}>
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
        {live?.item?.kind === "song" && liveSlides.some((s) => s.second.length > 0 || state?.lang_mode === "second") && (
          <label className="wp-inline-select">
            <span>Language</span>
            <select value={state?.lang_mode ?? "both"} onChange={(e) => void run(api.setLayout(undefined, e.target.value as LangMode))}>
              <option value="primary">{LANG_NAMES.en}</option>
              <option value="both">Both</option>
              <option value="second">{LANG_NAMES.fat}</option>
            </select>
          </label>
        )}
      </div>
      {!rehearsal && state?.auto && live?.item?.kind === "announcement" && <AutoBar state={state} onPause={(p) => void run(api.setAutoPaused(p))} />}
      {!rehearsal && live?.item?.kind === "song" && <RepeatBar state={state} slides={liveSlides} onSet={(label, total, done) => void run(api.setRepeat(label, total, done))} />}
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
        <span className="wp-wstabs__app">Worship Presenter</span>
        <span className="wp-spacer" />
        <button className="wp-btn wp-btn--quiet" onClick={() => setLayer("palette")}>
          Commands <kbd>Ctrl+K</kbd>
        </button>
      </nav>

      {activeTab === "service" ? (
        <header className={`wp-bar ${width < 960 ? "wp-bar--two" : ""}`}>
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
              placeholder="Quick scripture: Yohane 3:16"
              aria-label="Quick scripture. Enter puts the passage in Preview."
              onChange={(e) => setQuick(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && quick.trim()) e.preventDefault(), void quickStage();
                if (e.key === "Escape") e.preventDefault(), (e.target as HTMLInputElement).blur();
              }}
            />
          </label>
          <span className="wp-spacer" />
          <label className={`wp-switch ${rehearsal ? "is-on" : ""}`} title="Practise the service without sending anything to the screens">
            <input type="checkbox" checked={rehearsal} onChange={(e) => (setRehearsal(e.target.checked), setReh(null))} /> Rehearsal
          </label>
          <button className={`wp-btn wp-btn--quiet wp-music-chip ${music?.status.playing ? "is-on" : ""}`} onClick={() => setLayer(layer === "music" ? null : "music")} title={music?.status.title ?? "Background music"}>
            <Icon name="music" /> {music?.status.playing ? (music.status.paused ? "Paused" : music.status.ducked ? "Music (quiet)" : music.status.title ?? "Music") : "Music"}
          </button>
          <TimerChip timer={state?.timer ?? null} onClick={() => setLayer(layer === "timer" ? null : "timer")} />
          <button className={`wp-btn wp-btn--quiet ${state?.alert ? "is-on" : ""}`} onClick={() => setLayer(layer === "alert" ? null : "alert")}>
            <Icon name="alert" /> {state?.alert ? "Alert on" : "Alert"}
          </button>
          <ServiceActions state={state} latched={latched} onMode={setMode} onResume={resume} />
          <button
            className="wp-btn wp-btn--primary"
            onClick={() => void goLive()}
            disabled={!canGoLive}
            title={main.condition === "none" ? "Set up a main display first" : latched ? "Resume output first" : "Send the staged slide to the screens (F5)"}
          >
            Go live <kbd>F5</kbd>
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
          <button className={`wp-btn ${state?.mode === "blank" ? "is-pressed" : ""}`} aria-pressed={state?.mode === "blank"} onClick={() => setMode("blank")}>
            <Icon name={state?.mode === "blank" ? "check" : "blank"} /> Blank
          </button>
          <button className="wp-btn" onClick={() => setActiveTab("service")}>
            Return to service
          </button>
        </header>
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
          <ThemeEditor onClose={() => closeTab("themes")} />
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
            uiTheme={uiTheme}
            setUiTheme={setUiTheme}
            views={view}
            mainLive={!!live?.item && state?.mode === "live"}
            onOpenOutputs={() => setLayer("outputs")}
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
      {layer === "music" && <MusicPanel m={music} onClose={() => setLayer(null)} />}
      {layer === "alert" && <AlertPanel alert={state?.alert} onClose={() => setLayer(null)} />}
    </div>
  );
}

function ServiceActions({ state, latched, onMode, onResume }: { state: LiveState | undefined; latched: boolean; onMode: (m: Mode) => void; onResume: () => void }) {
  const btn = (m: Mode, label: string, icon: string) => (
    <button className={`wp-btn ${state?.mode === m ? "is-pressed" : ""}`} aria-pressed={state?.mode === m} onClick={() => onMode(m)}>
      <Icon name={state?.mode === m ? "check" : icon} /> {label}
    </button>
  );
  return (
    <div className="wp-service-actions" role="group" aria-label="Screens">
      {btn("blank", "Blank", "blank")}
      {btn("logo", "Logo", "logo")}
      {btn("clear", "Clear", "clear")}
      {latched && (
        <button className="wp-btn wp-btn--resume" onClick={onResume}>
          Resume output
        </button>
      )}
    </div>
  );
}

function TimerChip({ timer, onClick }: { timer: Timer | null; onClick: () => void }) {
  const now = useNow(!!timer, 500);
  const left = timer ? timerLeft(timer, now) : 0;
  return (
    <button className={`wp-btn wp-btn--quiet wp-num ${timer ? "is-on" : ""} ${timer && left < 0 ? "is-over" : ""}`} onClick={onClick}>
      <Icon name="timer" /> {timer ? fmtLeft(left) + (timer.paused_left_ms != null ? " paused" : "") : "Timer"}
    </button>
  );
}

function AutoBar({ state, onPause }: { state: LiveState; onPause: (paused: boolean) => void }) {
  const now = useNow(!state.auto?.paused, 500);
  const a = state.auto!;
  const d = a.durations_ms[state.slide_idx] ?? 0;
  const left = Math.max(0, Math.ceil((d - (now - state.slide_since_ms)) / 1000));
  const last = state.slide_idx >= a.durations_ms.length - 1 && !a.looping;
  return (
    <div className="wp-livebar">
      <span className="wp-num">{a.paused ? "Paused" : last ? "Last slide" : `Next slide in ${left} s`}</span>
      {a.looping && <span className="wp-note wp-note--inline">Loops</span>}
      <span className="wp-spacer" />
      <button className="wp-btn" onClick={() => onPause(!a.paused)}>
        {a.paused ? "Resume advancing" : "Pause advancing"}
      </button>
    </div>
  );
}

function RepeatBar({ state, slides, onSet }: { state: LiveState | undefined; slides: Slide[]; onSet: (label: string | null, total: number, done: number | null) => void }) {
  const labels = [...new Set(slides.map((s) => s.label))];
  const [label, setLabel] = useState(labels.find((l) => l.startsWith("C")) ?? labels[0] ?? "C");
  const [total, setTotal] = useState(3);
  const r = state?.repeat;
  if (!labels.length) return null;
  return (
    <div className="wp-livebar">
      {r ? (
        <>
          <span className="wp-num wp-livebar__count">
            {r.label}: {Math.min(r.done, 99)} of {r.total}
          </span>
          <button className="wp-icon-btn" aria-label="One fewer" onClick={() => onSet(r.label, r.total, Math.max(0, r.done - 1))}>
            −
          </button>
          <button className="wp-icon-btn" aria-label="One more" onClick={() => onSet(r.label, r.total, r.done + 1)}>
            +
          </button>
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--quiet" onClick={() => onSet(null, 1, null)}>
            Stop counting
          </button>
        </>
      ) : (
        <>
          <span>Count</span>
          <select className="wp-input" aria-label="Section to count" value={label} onChange={(e) => setLabel(e.target.value)}>
            {labels.map((l) => (
              <option key={l}>{l}</option>
            ))}
          </select>
          <span>×</span>
          <select className="wp-input" aria-label="Times" value={total} onChange={(e) => setTotal(Number(e.target.value))}>
            {[2, 3, 4, 5, 6, 8].map((n) => (
              <option key={n}>{n}</option>
            ))}
          </select>
          <button className="wp-btn wp-btn--quiet" onClick={() => onSet(label, total, null)} title="The stage display shows the count large for the band">
            Start counting
          </button>
        </>
      )}
    </div>
  );
}

function Clock() {
  const now = useNow(true, 1000);
  return <span className="wp-num">{new Date(now).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>;
}

function ViewToggle({ value, onChange }: { value: "list" | "thumbs"; onChange: (v: "list" | "thumbs") => void }) {
  return (
    <div className="wp-tabs ew-view" role="radiogroup" aria-label="Show slides as">
      <button role="radio" aria-checked={value === "list"} aria-selected={value === "list"} onClick={() => onChange("list")} title="Text list">
        List
      </button>
      <button role="radio" aria-checked={value === "thumbs"} aria-selected={value === "thumbs"} onClick={() => onChange("thumbs")} title="Pictures">
        Pictures
      </button>
    </div>
  );
}

/** A video or sound file in Preview: play, scrub and listen here, muted to begin with. */
function PreviewMedia({ item }: { item: MediaItem }) {
  const url = useMediaUrl();
  const src = url(`media:${item.path}`);
  return item.kind === "video" ? (
    <video className="ew-preview-media" src={src} controls muted playsInline preload="metadata" />
  ) : (
    <div className="ew-preview-media ew-preview-media--audio">
      <span>♪ {item.title}</span>
      <audio src={src} controls preload="metadata" />
    </div>
  );
}
