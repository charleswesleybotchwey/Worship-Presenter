// Talks to the Rust core through Tauri. When opened in a plain browser (vite dev),
// a small in-memory stand-in answers the same calls so the screens can be reviewed;
// it shares live state between tabs with BroadcastChannel, so #/output/main in a
// second tab behaves like an output window.

export type Mode = "live" | "blank" | "logo" | "clear";
export type LangMode = "primary" | "second" | "both";

export interface Slide {
  /** Scripture: this slide's reference as the display options want it ("" = none). */
  reference?: string | null;
  label: string;
  occurrence: number;
  first_key: number;
  last_key: number;
  primary: string[];
  second: string[];
  image?: string | null;
  /** Primary lines with inline chords (stage view only). */
  chords?: string[];
}

export interface LiveState {
  revision: number;
  item_idx: number | null;
  slide_idx: number;
  mode: Mode;
  lang_mode: LangMode;
  lines_per_slide: number;
  media_playing: boolean;
  media_restart: number;
  stage_message: string | null;
  timer: Timer | null;
  alert: Alert | null;
  auto: { durations_ms: number[]; looping: boolean; paused: boolean } | null;
  slide_since_ms: number;
  repeat: { label: string; total: number; done: number } | null;
  lining_out: boolean;
  subtitles?: "stream" | "screens" | "both" | null;
  transpose?: number;
  text_scale?: number;
  text_shift?: number;
  camera?: CameraLive | null;
}

export type TimerShow = "off" | "corner" | "full";
export interface Timer {
  label: string;
  ends_at_ms: number;
  paused_left_ms: number | null;
  count_over: boolean;
  on_screens: TimerShow;
  on_stage: boolean;
}
export interface Alert {
  text: string;
  style: "band" | "ticker";
  position: "top" | "bottom";
  until_ms: number | null;
  on_screens: boolean;
  on_stream: boolean;
  on_stage: boolean;
}
export interface TimerPreset {
  id: number | null;
  name: string;
  mode: "countdown" | "to_time";
  value: string;
}
export interface AlertPreset {
  id: number | null;
  text: string;
  position: "top" | "bottom";
  style: "band" | "ticker";
  seconds: number | null;
}

export type ProgramItem =
  | { kind: "song"; song_id: number; title: string; theme_id?: number | null; lyrics?: { lang: string; body: string }[] | null }
  | { kind: "scripture"; reference: string; versions: string[]; theme_id?: number | null }
  | { kind: "media"; media_id: number; title: string }
  | { kind: "announcement"; presentation_id: number; title: string }
  /** A divider in the service order ("Praise & Worship", "Sermon"): never on screen. */
  | { kind: "header"; title: string }
  /** A web page: shown in its own window over the main screen while live. */
  | { kind: "web"; url: string; title: string };

export interface DeckSlide {
  kind: "text" | "picture";
  title: string;
  body: string;
  image: string | null;
  seconds: number | null;
}
export interface Deck {
  id: number | null;
  title: string;
  looping: boolean;
  /** seconds per slide when advancing by itself; 0 = only on Next */
  seconds: number;
  slides: DeckSlide[];
}
export interface DeckSummary {
  id: number;
  title: string;
  slides: number;
  looping: boolean;
}

export interface MediaItem {
  id: number;
  kind: "image" | "video" | "audio";
  path: string;
  title: string;
  duration_ms: number | null;
  width: number | null;
  height: number | null;
  thumb: string | null;
  tags: string | null;
  bytes: number;
  missing: boolean;
  /** EasyWorship's Repeating: "loop" or "once" (default once). */
  repeat?: "loop" | "once" | null;
  /** EasyWorship's Aspect Ratio: "fit" (default), "fill" (crop) or "stretch". */
  fit?: "fit" | "fill" | "stretch" | null;
}

export interface ImportReview {
  name: string;
  read: number;
  recognised: number;
  ambiguous: { item: string; reason: string }[];
  rejected: { item: string; reason: string }[];
  duplicates: string[];
  notes: string[];
}

export interface Broadcast {
  state: LiveState;
  item: ProgramItem | null;
  slides: Slide[];
  caption: string | null;
  theme_rev: number;
  media: MediaItem | null;
  /** Outputs that came back after being lost; black until resumed. */
  held: string[];
  song_key?: string | null;
  rule_theme_id?: number | null;
  song_copyright?: string | null;
}

export interface SongHit {
  id: number;
  title: string;
  author: string | null;
  numbers: string[];
  first_line: string | null;
}

export interface Version {
  id: number;
  code: string;
  name: string;
  lang: string | null;
}

export interface ScheduleSummary {
  id: number;
  name: string;
  service_date: string | null;
  template: boolean;
  items: number;
}

export interface Schedule {
  id: number;
  name: string;
  service_date: string | null;
  template: boolean;
  items: ProgramItem[];
}

export interface ImportReport {
  name: string;
  read: number;
  imported: number;
  skipped: number;
  fixed_chars: number;
  notes: string[];
}

import { type ActiveThemes, type Theme, type ThemeKind, type ThemeRow } from "./theme/types";

export interface SongDoc {
  id: number | null;
  title: string;
  author: string | null;
  copyright: string | null;
  ccli: string | null;
  numbers: { hymnal: string; number: string }[];
  lyrics: { lang: string; body: string }[];
  sequence: string;
  lines_per_slide: number | null;
  /** Musical key, for the musicians' stage view. */
  key?: string | null;
}

export interface BackupInfo {
  path: string;
  file: string;
  taken: string;
  reason: string;
  bytes: number;
}

export interface BackupContents {
  songs: number;
  bibles: number;
  schedules: number;
}

export interface Profile {
  slug: string;
  name: string;
  active: boolean;
  songs: number;
  bibles: number;
}

export type ImportKind = "ew-songs" | "ewb" | "bibleshow";

export interface RemoteStatus {
  running: boolean;
  url: string | null;
  stage_url: string | null;
  pin: string | null;
  qr_svg: string | null;
  lan_ip: string | null;
  sessions: { id: string; role: "operator" | "stage" | "viewer"; device: string; ip: string; joined_secs_ago: number; idle_secs: number }[];
  log: { at: number; device: string; action: string; ok: boolean }[];
  /** Lyrics on the congregation's phones (read-only, no PIN). */
  follow?: boolean;
  follow_url?: string | null;
  follow_qr_svg?: string | null;
}

export interface ReplaceOptions {
  match_case: boolean;
  whole_words: boolean;
  in_words: boolean;
  in_titles: boolean;
}

export interface ItemCollection {
  id: number;
  kind: "scripture" | "media";
  name: string;
  count: number;
}

export interface ThemeRule {
  id: number;
  kind: "song" | "scripture";
  match: "hymnal" | "language" | "bilingual" | "interval" | "version";
  value: string;
  theme_id: number;
}

export interface ForeignDesign {
  name: string;
  kind: "song" | "scripture" | "lower_third" | "announcement";
  preview: Record<string, unknown>;
  notes: string[];
  suggested: boolean;
}

export interface CameraLive {
  device_id: string;
  label: string;
  mode: "full" | "behind" | "corner";
}

export interface PulpitNote {
  id: number;
  device: string;
  text: string;
  deck_id: number;
  deck_title: string;
  slide_idx: number;
}

export interface HeardRef {
  reference: string;
  book_no: number;
  chapter: number;
  verse_from: number | null;
  verse_to: number | null;
  heard: string;
  at: number;
}
export interface SpeechStatus {
  listening: string | null;
  caption: string;
  recent: HeardRef[];
  problem: string | null;
  vosk_ready: boolean;
  installing: string | null;
  windows_speech: boolean;
  whisper_models?: string[];
  /** Loading a model or connecting. */
  starting?: boolean;
  /** Microphone level as the engine hears it, 0–1. */
  level?: number;
  /** This session's transcript: [time, words, confidence 0–1 or null]. */
  lines?: [string, string, number | null][];
  lines_total?: number;
  warning?: string | null;
  idle_paused?: boolean;
  gain?: number;
}

export interface FontFamily {
  name: string;
  ghanaian: boolean;
  styles: number;
}

export interface OutputConfig {
  role: string;
  name: string;
  display_index: number | null;
  display_name: string | null;
  custom: { x: number; y: number; w: number; h: number } | null;
  margins: { left: number; right: number; top: number; bottom: number };
  alpha: boolean;
  song_theme_id: number | null;
  scripture_theme_id: number | null;
  /** EasyWorship's "NDI Stream" output monitor (null: a monitor). */
  ndi?: { width: number; height: number; fps: number } | null;
  on_logo: "logo" | "blank" | "transparent" | "ignore";
  on_blank: "blank" | "transparent" | "ignore";
  show_on_startup: boolean;
}
export interface ServiceInterval {
  id: number | null;
  name: string;
  weekday: number;
  start: string;
  end: string;
}
export interface UsageRow {
  title: string;
  numbers: string;
  author: string;
  copyright: string;
  ccli: string;
  interval: string;
  uses: number;
  first: string;
  last: string;
}

export interface VerseHit {
  version: string;
  reference: string;
  book_no: number;
  chapter: number;
  verse: number;
  text: string;
  /** Found through another Bible (English words, Fante Bible chosen). */
  found_in?: string | null;
}

export interface SongRow {
  id: number;
  number: string | null;
  title: string;
  first_line: string | null;
  author: string | null;
  copyright: string | null;
  langs: string[];
  sort_key: string;
}
export interface SongTree {
  all: number;
  collections: { key: string; name: string; count: number }[];
  mine: { key: string; name: string; count: number }[];
}
export type SongScope = { kind: "all" } | { kind: "hymnal"; code: string } | { kind: "mine"; id: number };
export interface ChapterView {
  version: string;
  book: string;
  book_no: number;
  chapter: number;
  chapters: number;
  rows: { verse: number; reference: string; text: string; heading: string | null; selected: boolean }[];
}

export interface MusicRules {
  on_live: "keep" | "duck" | "fade_out";
  with_announcements: boolean;
  crossfade_ms: number;
  looping: boolean;
}
export interface MusicStatus {
  started: boolean;
  status: { playing: boolean; paused: boolean; title: string | null; index: number | null; position_ms: number; length_ms: number | null; volume: number; ducked: boolean; problem: string | null };
  rules: MusicRules;
  playlist_ids: number[];
}

export interface NdiStatus {
  runtime: string | null;
  problem: string | null;
  capture_supported: boolean;
  sources: { source: string; name: string; seconds: number; fps: number; captured: number; sent: number; process_ms: number; connections: number; width: number; height: number; audio?: boolean }[];
}

export interface OutputStatus {
  role: string;
  display_index: number | null;
  /** Revision the output last drew, and seconds since it last answered. */
  revision: number;
  silent_secs: number;
}

export interface Display {
  index: number;
  name: string;
  width: number;
  height: number;
  primary: boolean;
}

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// One request for the four things the service window watches (outputs, speech,
// recording, pulpit notes): the first call in a 0.25 s window fetches them all, the
// others share that answer. The browser preview's stand-in answers each separately.
type StatusAll = { outputs: OutputStatus[]; speech: SpeechStatus; stream: Awaited<ReturnType<typeof api.streamStatusOne>>; notes: PulpitNote[] };
let statusBatch: { at: number; p: Promise<StatusAll> } | null = null;
function statusAll(): Promise<StatusAll> {
  if (statusBatch && Date.now() - statusBatch.at < 250) return statusBatch.p;
  const p = call<StatusAll>("status_all", { knownLines: speechLines.total >= 0 ? speechLines.total : null });
  statusBatch = { at: Date.now(), p };
  p.catch(() => (statusBatch = null));
  return p;
}

// the transcript lines the window already has (only new ones are sent)
let speechLines: { total: number; lines: [string, string, number | null][] } = { total: -1, lines: [] };

// a request that fails is written to the log once per kind (many are polled quietly, so
// a refused request, e.g. a wrong argument, would otherwise go unseen)
const failedOnce = new Set<string>();

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      return await invoke<T>(cmd, args);
    } catch (e) {
      if (cmd !== "log_error" && !failedOnce.has(cmd)) {
        failedOnce.add(cmd);
        const where = location.hash.replace(/^#\/?/, "") || "control";
        void invoke("log_error", { window: where, text: `${cmd} failed: ${String(e instanceof Error ? e.message : e).slice(0, 300)}` }).catch(() => {});
      }
      throw e;
    }
  }
  // the browser preview's stand-in, in its own file (never loaded by the real app)
  const { mock } = await import("./api.mock");
  return mock(cmd, args ?? {}) as Promise<T>;
}

export const api = {
  searchSongs: (query: string) => call<SongHit[]>("search_songs", { query }),
  previewSong: (songId: number, title: string) => call<Slide[]>("preview_song", { songId, title }),
  versions: () => call<Version[]>("bible_versions"),
  previewScripture: (reference: string, versions: string[]) =>
    call<[Slide[], string | null]>("preview_scripture", { reference, versions }),
  goLive: (item: ProgramItem, slideIdx: number) => call<LiveState>("go_live", { item, slideIdx }),
  show: (slideIdx: number) => call<LiveState>("show_slide", { slideIdx }),
  step: (delta: number) => call<LiveState>("show_slide", { delta }),
  setMode: (mode: Mode) => call<LiveState>("set_mode", { mode }),
  setLayout: (lines?: number, langMode?: LangMode) => call<LiveState>("set_layout", { lines, langMode }),
  undoGoLive: () => call<LiveState>("undo_go_live"),
  resync: () => call<void>("resync"),
  displays: () => call<Display[]>("list_displays"),
  openOutput: (role: string, displayIndex: number | null) => call<void>("open_output", { role, displayIndex }),
  closeOutput: (role: string) => call<void>("close_output", { role }),
  screensPower: (on: boolean) => call<boolean>("screens_power", { on }),
  dashboard: () => call<{ tiles: { role: string; name: string; kind: string; state: string; detail: string }[]; screens_off: boolean }>("dashboard"),
  systemCheck: () => call<{ id: string; label: string; status: "ok" | "warn" | "bad"; detail: string; fix: string | null }[]>("system_check"),
  screensAreOn: () => call<boolean>("screens_are_on"),
  outputAck: (role: string, revision: number) => call<void>("output_ack", { role, revision }),
  outputStatus: (): Promise<OutputStatus[]> => (inTauri ? statusAll().then((x) => x.outputs) : call<OutputStatus[]>("output_status")),
  releaseOutput: (role: string) => call<void>("release_output", { role }),
  listSchedules: () => call<ScheduleSummary[]>("list_schedules"),
  openSchedule: (id: number) => call<Schedule>("open_schedule", { id }),
  newSchedule: (name: string, serviceDate: string | null, template: boolean, fromTemplate: number | null) =>
    call<Schedule>("new_schedule", { name, serviceDate, template, fromTemplate }),
  saveSchedule: (id: number, items: ProgramItem[]) => call<void>("save_schedule", { id, items }),
  deleteSchedule: (id: number) => call<void>("delete_schedule", { id }),
  lastSchedule: () => call<number | null>("last_schedule"),
  importFile: (kind: ImportKind, path: string, code: string | null, skip: string[] | null = null) => call<ImportReport>("import_file", { kind, path, code, skip }),
  getSong: (id: number) => call<SongDoc>("get_song", { id }),
  previewSongDoc: (doc: SongDoc) => call<Slide[]>("preview_song_doc", { doc }),
  saveSong: (doc: SongDoc) => call<{ id: number; live: boolean }>("save_song", { doc }),
  refreshLiveItem: (title: string | null) => call<void>("refresh_live_item", { title }),
  importReview: (kind: ImportKind, path: string, code: string | null) => call<ImportReview>("import_review", { kind, path, code }),
  songFilesReview: (paths: string[]) => call<ImportReview>("song_files_review", { paths }),
  songFilesImport: (paths: string[], skip: string[] | null) => call<ImportReport>("song_files_import", { paths, skip }),
  relinkMedia: (id: number, path: string) => call<MediaItem>("relink_media", { id, path }),
  identifyOutput: (role: string) => call<void>("identify_output", { role }),
  identifyMonitors: () => call<number>("identify_monitors"),
  getSettings: () => call<Record<string, string>>("get_settings"),
  setSetting: (key: string, value: string) => call<void>("set_setting", { key, value }),
  deleteSong: (id: number) => call<void>("delete_song", { id }),
  listBackups: () => call<BackupInfo[]>("list_backups"),
  backupNow: () => call<BackupInfo>("backup_now"),
  inspectBackup: (path: string) => call<BackupContents>("inspect_backup", { path }),
  restoreBackup: (path: string) => call<BackupInfo>("restore_backup", { path }),
  backupsFolder: () => call<string>("backups_folder"),
  activeThemes: () => call<ActiveThemes>("active_themes"),
  listThemes: () => call<ThemeRow[]>("list_themes"),
  saveTheme: (id: number | null, theme: Theme) => call<number>("save_theme", { id, theme }),
  duplicateTheme: (id: number) => call<number>("duplicate_theme", { id }),
  deleteTheme: (id: number) => call<void>("delete_theme", { id }),
  setDefaultTheme: (kind: ThemeKind, id: number) => call<void>("set_default_theme", { kind, id }),
  listMedia: (kind: MediaItem["kind"] | null, query: string) => call<MediaItem[]>("list_media", { kind, query }),
  importMedia: (paths: string[]) => call<{ file: string; item: MediaItem | null; error: string | null }[]>("import_media", { paths }),
  setMediaMeta: (id: number, durationMs: number | null, width: number | null, height: number | null) => call<void>("set_media_meta", { id, durationMs, width, height }),
  saveMediaThumb: (id: number, jpeg: number[]) => call<string>("save_media_thumb", { id, jpeg }),
  renameMedia: (id: number, title: string, tags: string | null) => call<void>("rename_media", { id, title, tags }),
  mediaUsedBy: (id: number) => call<string[]>("media_used_by", { id }),
  themeRules: () => call<ThemeRule[]>("theme_rules"),
  themeGallery: () => call<Theme[]>("theme_gallery"),
  syncStatus: () => call<{ folder: string | null; role: "share" | "follow" | null; shared_ms: number | null; taken_ms: number | null; waiting: [string, number] | null; computer: string }>("sync_status"),
  syncSet: (folder: string | null, role: string | null) => call<void>("sync_set", { folder, role }),
  syncShareNow: () => call<number>("sync_share_now"),
  syncTakeNow: () => call<string>("sync_take_now"),
  streamStatus: () => (inTauri ? statusAll().then((x) => x.stream) : api.streamStatusOne()),
  streamStatusOne: () => call<{ ffmpeg: boolean; running: boolean; seconds: number; file: string | null; destinations: string[]; report: string; installing: string | null }>("stream_status"),
  streamInstallFfmpeg: () => call<void>("stream_install_ffmpeg"),
  streamStart: (req: { record: boolean; folder: string | null; destinations: { kind: string; target: string }[] }) => call<string>("stream_start", { req }),
  streamStop: () => call<string | null>("stream_stop"),
  onlineKeySet: (service: string, value: string) => call<void>("online_key_set", { service, value }),
  onlineKeysSaved: () => call<Record<string, boolean>>("online_keys_saved"),
  onlineSearch: (source: string, query: string, page: number) => call<{ source: string; id: string; kind: string; title: string; thumb: string; file: string; width: number; height: number; author: string; author_url: string; page_url: string; track: string | null }[]>("online_search", { source, query, page }),
  onlineDownload: (item: unknown) => call<MediaItem>("online_download", { item }),
  onlineOpen: (url: string) => call<void>("online_open", { url }),
  streamKeySet: (kind: string, value: string) => call<void>("stream_key_set", { kind, value }),
  streamKeysSaved: () => call<Record<string, boolean>>("stream_keys_saved"),
  updateCheck: () => call<{ version: string; notes: string | null } | null>("update_check"),
  updateInstall: () => call<void>("update_install"),
  logError: (window: string, text: string) => call<void>("log_error", { window, text }),
  openLogs: () => call<void>("open_logs"),
  diagnosticReport: () => call<string>("diagnostic_report"),
  lyricsSearch: (title: string, artist: string) => call<{ source: string; key: string; title: string; author: string; words: string | null }[]>("lyrics_search", { title, artist }),
  lyricsWords: (source: string, key: string) => call<string>("lyrics_words", { source, key }),
  lyricsAdd: (title: string, author: string, words: string, source: string) => call<number>("lyrics_add", { title, author, words, source }),
  bibleCatalogue: () => call<{ id: string; language: string; title: string }[]>("bible_catalogue"),
  bibleDownload: (id: string, code: string, title: string, lang: string) => call<number>("bible_download", { id, code, title, lang }),
  importEws: (path: string) => call<{ title: string; items: ProgramItem[]; notes: string[] }>("import_ews", { path }),
  songsReplacePlan: (find: string, withText: string, options: ReplaceOptions) => call<{ song_id: number; title: string; count: number; before: string; after: string }[]>("songs_replace_plan", { find, with: withText, options }),
  songsReplaceApply: (find: string, withText: string, options: ReplaceOptions) => call<number>("songs_replace_apply", { find, with: withText, options }),
  smartRules: (id: number) => call<{ all: boolean; rules: { field: string; op: string; value: string }[] } | null>("smart_rules", { id }),
  smartRulesSet: (id: number, rules: { all: boolean; rules: { field: string; op: string; value: string }[] } | null) => call<void>("smart_rules_set", { id, rules }),
  itemCollections: (kind: "scripture" | "media") => call<ItemCollection[]>("item_collections", { kind }),
  itemCollectionCreate: (kind: "scripture" | "media", name: string) => call<number>("item_collection_create", { kind, name }),
  itemCollectionRename: (id: number, name: string) => call<void>("item_collection_rename", { id, name }),
  itemCollectionDelete: (id: number) => call<void>("item_collection_delete", { id }),
  itemCollectionAdd: (id: number, item: ProgramItem) => call<void>("item_collection_add", { id, item }),
  itemCollectionRemove: (id: number, item: ProgramItem) => call<void>("item_collection_remove", { id, item }),
  itemCollectionItems: (id: number) => call<ProgramItem[]>("item_collection_items", { id }),
  outlineDeck: (text: string, version: string) => call<Deck>("outline_deck", { text, version }),
  quickSong: (typed: string) => call<SongHit[]>("quick_song", { typed }),
  saveThemeRules: (rules: ThemeRule[]) => call<void>("save_theme_rules", { rules }),
  resolveTheme: (item: ProgramItem) => call<number | null>("resolve_theme", { item }),
  setMediaOptions: (id: number, repeat: "loop" | "once" | null, fit: "fit" | "fill" | "stretch" | null) => call<void>("set_media_options", { id, repeat, fit }),
  revealMedia: (id: number) => call<void>("reveal_media", { id }),
  deleteMedia: (id: number) => call<void>("delete_media", { id }),
  mediaControl: (playing: boolean | null, restart: boolean) => call<LiveState>("media_control", { playing, restart }),
  getTheme: (id: number) => call<Theme>("get_theme", { id }),
  exportTheme: (id: number, path: string) => call<number>("export_theme", { id, path }),
  importTheme: (path: string) => call<{ id: number; name: string }>("import_theme", { path }),
  setStageMessage: (message: string | null) => call<LiveState>("set_stage_message", { message }),
  timerStart: (label: string, mode: TimerPreset["mode"], value: string, onScreens: TimerShow, onStage: boolean) =>
    call<LiveState>("timer_start", { label, mode, value, onScreens, onStage }),
  timerControl: (action: "pause" | "resume" | "add" | "stop" | "show", opts: { ms?: number; onScreens?: TimerShow; onStage?: boolean } = {}) =>
    call<LiveState>("timer_control", { action, ms: opts.ms ?? null, onScreens: opts.onScreens ?? null, onStage: opts.onStage ?? null }),
  alertShow: (a: { text: string; values: [string, string][]; style: Alert["style"]; position: Alert["position"]; seconds: number | null; onScreens: boolean; onStream: boolean; onStage: boolean }) =>
    call<LiveState>("alert_show", a),
  alertClear: () => call<LiveState>("alert_clear"),
  cuePresets: () => call<{ timers: TimerPreset[]; alerts: AlertPreset[] }>("cue_presets"),
  saveTimerPreset: (preset: TimerPreset) => call<number>("save_timer_preset", { preset }),
  saveAlertPreset: (preset: AlertPreset) => call<number>("save_alert_preset", { preset }),
  deleteCuePreset: (kind: "timer" | "alert", id: number) => call<void>("delete_cue_preset", { kind, id }),
  listAnnouncements: () => call<DeckSummary[]>("list_announcements"),
  getAnnouncement: (id: number) => call<Deck>("get_announcement", { id }),
  announcementTemplates: () => call<Deck[]>("announcement_templates"),
  saveAnnouncement: (deck: Deck) => call<number>("save_announcement", { deck }),
  deleteAnnouncement: (id: number) => call<void>("delete_announcement", { id }),
  previewAnnouncement: (id: number) => call<Slide[]>("preview_announcement", { id }),
  setAutoPaused: (paused: boolean) => call<LiveState>("set_auto_paused", { paused }),
  setRepeat: (label: string | null, total: number, done: number | null = null) => call<LiveState>("set_repeat", { label, total, done }),
  setLiningOut: (on: boolean) => call<LiveState>("set_lining_out", { on }),
  listFonts: () => call<FontFamily[]>("list_fonts"),
  foreignThemeReview: (path: string) => call<ForeignDesign[]>("foreign_theme_review", { path }),
  foreignThemeImport: (path: string, chosen: number[]) => call<{ id: number; name: string }[]>("foreign_theme_import", { path, chosen }),
  setTextAdjust: (scale: number, shift: number) => call<LiveState>("set_text_adjust", { scale, shift }),
  setTranspose: (semitones: number) => call<LiveState>("set_transpose", { semitones }),
  setCamera: (camera: CameraLive | null) => call<LiveState>("set_camera", { camera }),
  setSubtitles: (target: "stream" | "screens" | "both" | null) => call<LiveState>("set_subtitles", { target }),
  pulpitNotes: (): Promise<PulpitNote[]> => (inTauri ? statusAll().then((x) => x.notes) : call<PulpitNote[]>("pulpit_notes")),
  pulpitDismiss: (id: number | null) => call<void>("pulpit_dismiss", { id }),
  // the transcript is only sent when it has grown; the window keeps its own copy
  speechStatus: async (): Promise<SpeechStatus> => {
    const st = inTauri ? { ...(await statusAll()).speech } : await call<SpeechStatus>("speech_status", { knownLines: speechLines.total >= 0 ? speechLines.total : null });
    if (st.lines_total != null && st.lines_total === speechLines.total && (st.lines ?? []).length === 0 && speechLines.total > 0) st.lines = speechLines.lines;
    else if (st.lines_total != null) speechLines = { total: st.lines_total, lines: st.lines ?? [] };
    return st;
  },
  speechStart: (engine: string, mic: string | null, referencesOnly: boolean) => call<void>("speech_start", { engine, mic, referencesOnly }),
  speechStop: () => call<void>("speech_stop"),
  speechDismiss: (reference: string | null) => call<void>("speech_dismiss", { reference }),
  speechMics: () => call<string[]>("speech_mics"),
  speechSetGain: (percent: number) => call<void>("speech_set_gain", { percent }),
  speechHistory: () => call<{ name: string; bytes: number }[]>("speech_history"),
  speechHistoryRead: (name: string) => call<string>("speech_history_read", { name }),
  speechHistoryDelete: (name: string) => call<void>("speech_history_delete", { name }),
  speechInstallVosk: () => call<void>("speech_install_vosk"),
  speechInstallWhisper: (model: string) => call<void>("speech_install_whisper", { model }),
  speechSaveNotes: (path: string) => call<number>("speech_save_notes", { path }),
  speechTestRecording: (path: string) => call<{ transcript: [number, string][]; found: [number, string, string][]; model: string; seconds: number; took_seconds: number }>("speech_test_recording", { path }),
  presentationImport: (path: string) =>
    call<{ stage: "done" | "render_pdf"; method: string; title: string; deck_id: number | null; pdf: string | null; token: string | null; warnings: string[] }>("presentation_import", { path }),
  /** A drawn page as raw bytes (not JSON), so big decks import quickly. */
  presentationPage: async (token: string, index: number, bytes: Uint8Array, ext: "jpg" | "png"): Promise<void> => {
    if (!inTauri) return void (await import("./api.mock")).mockPage(token, index, bytes, ext);
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("presentation_page", bytes, { headers: { "x-token": token, "x-index": String(index), "x-ext": ext } });
  },
  presentationFinish: (token: string, title: string) => call<number>("presentation_finish", { token, title }),
  outputConfigs: () => call<OutputConfig[]>("output_configs"),
  outputConfig: (role: string) => call<OutputConfig>("output_config", { role }),
  saveOutputConfig: (config: OutputConfig) => call<void>("save_output_config", { config }),
  serviceIntervals: () => call<ServiceInterval[]>("service_intervals"),
  saveServiceInterval: (interval: ServiceInterval) => call<number>("save_service_interval", { interval }),
  deleteServiceInterval: (id: number) => call<void>("delete_service_interval", { id }),
  currentServiceInterval: () => call<string | null>("current_service_interval"),
  usageReport: (from: string, to: string) => call<UsageRow[]>("usage_report", { from, to }),
  saveUsageReport: (from: string, to: string, path: string) => call<number>("save_usage_report", { from, to, path }),
  songTree: () => call<SongTree>("song_tree"),
  browseSongs: (scope: SongScope, query: string) => call<SongRow[]>("browse_songs", { scope, query }),
  collectionCreate: (name: string) => call<number>("collection_create", { name }),
  collectionRename: (id: number, name: string) => call<void>("collection_rename", { id, name }),
  collectionDelete: (id: number) => call<void>("collection_delete", { id }),
  collectionAdd: (id: number, songIds: number[]) => call<number>("collection_add", { id, songIds }),
  collectionRemove: (id: number, songIds: number[]) => call<void>("collection_remove", { id, songIds }),
  browseChapter: (version: string, typed: string) => call<ChapterView>("browse_chapter", { version, typed }),
  rebuildSearch: () => call<number>("rebuild_search"),
  searchBible: (versions: string[], query: string) => call<VerseHit[]>("search_bible", { versions, query }),
  resetSongLooks: () => call<number>("reset_song_looks"),
  musicStatus: () => call<MusicStatus>("music_status"),
  musicSetPlaylist: (mediaIds: number[]) => call<void>("music_set_playlist", { mediaIds }),
  musicPlay: (index: number | null) => call<void>("music_play", { index }),
  musicPause: (paused: boolean) => call<void>("music_pause", { paused }),
  musicNext: () => call<void>("music_next"),
  musicPrevious: () => call<void>("music_previous"),
  musicStop: () => call<void>("music_stop"),
  musicVolume: (volume: number) => call<void>("music_volume", { volume }),
  musicUnduck: () => call<void>("music_unduck"),
  musicSetRules: (rules: MusicRules) => call<void>("music_set_rules", { rules }),
  remoteStatus: () => call<RemoteStatus>("remote_status"),
  remoteStart: (everywhere: boolean, port: number) => call<void>("remote_start", { everywhere, port }),
  remoteStop: () => call<void>("remote_stop"),
  remoteFollow: (on: boolean) => call<void>("remote_follow", { on }),
  remoteControlKey: (on: boolean) => call<string | null>("remote_control_key", { on }),
  remoteControlKeyGet: () => call<string | null>("remote_control_key_get"),
  midiPorts: () => call<string[]>("midi_ports"),
  midiUse: (port: string | null) => call<void>("midi_use", { port }),
  midiLearn: (action: string) => call<void>("midi_learn", { action }),
  midiForget: (action: string) => call<void>("midi_forget", { action }),
  midiStatus: () => call<{ port: string | null; learning: string | null; map: Record<string, string> }>("midi_status"),
  remoteNewPin: () => call<string>("remote_new_pin"),
  remoteRevoke: (id: string) => call<void>("remote_revoke", { id }),
  remoteEndAll: () => call<void>("remote_end_all"),
  ndiStatus: () => call<NdiStatus>("ndi_status"),
  ndiTestPattern: (name: string, seconds: number) => call<void>("ndi_test_pattern", { name, seconds }),
  ndiStart: (source: "lower_third" | "main", name: string, fps: number, groups?: string, renderer?: "builtin" | "capture") => call<void>("ndi_start", { source, name, fps, groups: groups || null, renderer: renderer ?? null }),
  openNdiRuntimePage: () => call<void>("open_ndi_runtime_page"),
  setWindowLook: (dark: boolean) => call<void>("set_window_look", { dark }),
  showMainWindow: () => call<void>("show_main_window"),
  ndiAudio: (source: string, on: boolean) => call<void>("ndi_audio", { source, on }),
  ndiSources: (groups?: string) => call<{ name: string; url: string }[]>("ndi_sources", { groups: groups || null }),
  ndiStop: (source: string) => call<void>("ndi_stop", { source }),
  listProfiles: () => call<Profile[]>("list_profiles"),
  currentProfile: () => call<{ slug: string; name: string }>("current_profile"),
  createProfile: (name: string, copyCurrent: boolean) => call<string>("create_profile", { name, copyCurrent }),
  renameProfile: (slug: string, name: string) => call<void>("rename_profile", { slug, name }),
  deleteProfile: (slug: string) => call<string>("delete_profile", { slug }),
  switchProfile: (slug: string) => call<void>("switch_profile", { slug }),
};

export async function onProfileChanged(cb: () => void): Promise<() => void> {
  if (!inTauri) {
    window.addEventListener("wp-profile-changed", cb);
    return () => window.removeEventListener("wp-profile-changed", cb);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen("profile-changed", cb);
}

let mediaRootCache: string | null = null;
/** URL a window can load for a path inside the profile's media folder. */
export async function mediaUrlResolver(): Promise<(rel: string) => string> {
  if (!inTauri) return (rel) => rel;
  const { invoke, convertFileSrc } = await import("@tauri-apps/api/core");
  mediaRootCache ??= await invoke<string>("media_root");
  const root = mediaRootCache;
  return (rel) => convertFileSrc(`${root}/${rel}`);
}
/** Forget the cached folder (profile switched). */
export function resetMediaRoot() {
  mediaRootCache = null;
}

export async function pickMediaFiles(): Promise<string[]> {
  if (!inTauri) return ["/samples/sunrise.jpg"];
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({
    multiple: true,
    title: "Choose pictures, videos or sound files",
    filters: [{ name: "Pictures, videos and sound", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp", "mp4", "m4v", "webm", "mov", "mp3", "wav", "ogg", "m4a", "aac", "flac"] }],
  });
  return Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
}

/** Where to save an exported theme. */
export async function pickThemeSave(name: string): Promise<string | null> {
  if (!inTauri) return `${name}.wptheme`;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ title: "Export theme", defaultPath: `${name}.wptheme`, filters: [{ name: "Worship Presenter theme", extensions: ["wptheme"] }] })) ?? null;
}

export async function pickEwsOpen(): Promise<string | null> {
  if (!inTauri) return "Sunday.ewsx";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title: "Open an EasyWorship schedule", filters: [{ name: "EasyWorship 7 schedule", extensions: ["ewsx"] }] });
  return typeof r === "string" ? r : null;
}

export async function pickThemeOpen(): Promise<string | null> {
  if (!inTauri) return "Sunrise.wptheme";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({
    title: "Import a theme",
    filters: [
      { name: "Themes (Worship Presenter, EasyWorship, ProPresenter)", extensions: ["wptheme", "ewtx", "proTheme", "protheme"] },
      { name: "Worship Presenter theme", extensions: ["wptheme"] },
      { name: "EasyWorship theme", extensions: ["ewtx"] },
      { name: "ProPresenter theme", extensions: ["proTheme", "protheme"] },
    ],
  });
  return typeof r === "string" ? r : null;
}

/** Picks a backup file to restore from (any folder, e.g. a USB stick). */
export async function pickBackup(): Promise<string | null> {
  if (!inTauri) return "E:\\library-20261101-0930-manual.db";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title: "Choose a Worship Presenter backup", filters: [{ name: "Library backup", extensions: ["db"] }] });
  return typeof r === "string" ? r : null;
}

export async function onOutputConfigChanged(cb: (role: string) => void): Promise<() => void> {
  if (!inTauri) {
    const ch = new BroadcastChannel("wp-live");
    ch.onmessage = (e) => e.data.type === "output-config" && cb(e.data.role);
    return () => ch.close();
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("output-config-changed", (e) => cb(e.payload));
}

/** Song files from other programs: several files, or a folder. */
export async function pickSongFiles(folder: boolean): Promise<string[]> {
  if (!inTauri) return folder ? ["C:\\Songs\\OpenLP export"] : ["Way Maker.txt", "Amazing Grace.xml"];
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = folder
    ? await open({ directory: true, title: "Choose a folder of song files" })
    : await open({
        multiple: true,
        title: "Choose song files",
        filters: [{ name: "Song files", extensions: ["xml", "pro6", "pro", "txt", "usr", "cho", "chopro", "chordpro", "crd"] }, { name: "All files (OpenSong songs have no extension)", extensions: ["*"] }],
      });
  return Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
}

export async function pickPresentation(): Promise<string | null> {
  if (!inTauri) return "Harvest Thanksgiving.pptx";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title: "Choose a PowerPoint or PDF", filters: [{ name: "PowerPoint or PDF", extensions: ["pptx", "ppt", "pps", "ppsx", "odp", "pdf"] }] });
  return typeof r === "string" ? r : null;
}

/** An EasyWorship or ProPresenter theme file. */
/** The Windows menu bar: each choice arrives as its id ("file.save", "live.next"…). */
export function onMenu(f: (id: string) => void): () => void {
  if (!inTauri) {
    // the browser preview: window.__wpMenu("live.next") stands in for the menu
    (window as unknown as { __wpMenu?: (id: string) => void }).__wpMenu = f;
    return () => {};
  }
  let un = () => {};
  import("@tauri-apps/api/event").then(({ listen }) => listen<string>("menu", (e) => f(e.payload)).then((u) => (un = u)));
  return () => un();
}

/** Address of the newest picture from an NDI source (served by the app over ndi://). */
export async function ndiFrameUrl(name: string, url: string): Promise<string> {
  if (!inTauri) return "/samples/sunrise.jpg";
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  const base = convertFileSrc("x", "ndi").replace(/x$/, "");
  return `${base}frame/${encodeURIComponent(name)}?url=${encodeURIComponent(url)}`;
}

/** A sermon recording, for testing scripture recognition. */
export async function pickRecording(): Promise<string | null> {
  if (!inTauri) return "sermon.mp3";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title: "Choose a sermon recording", filters: [{ name: "Recordings", extensions: ["mp3", "wav", "m4a", "aac", "flac", "ogg"] }] });
  return typeof r === "string" ? r : null;
}

export async function pickForeignTheme(): Promise<string | null> {
  if (!inTauri) return "Theme.proTheme";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title: "Choose an EasyWorship or ProPresenter theme", filters: [{ name: "EasyWorship or ProPresenter theme", extensions: ["ewtx", "proTheme", "protheme"] }] });
  return typeof r === "string" ? r : null;
}

export async function pickCsvSave(name: string): Promise<string | null> {
  if (!inTauri) return name;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ title: "Save the song usage report", defaultPath: name, filters: [{ name: "Spreadsheet (CSV)", extensions: ["csv"] }] })) ?? null;
}

/** Actions from button boxes and MIDI pads that the service window carries out. */
export async function onControlAction(cb: (a: string) => void): Promise<() => void> {
  if (!inTauri) {
    (window as unknown as { __wpControl?: (a: string) => void }).__wpControl = cb;
    return () => {};
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("control-action", (e) => cb(e.payload));
}

/** The shared folder for sync. */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return "D:\\OneDrive\\Church";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ title: "Choose the shared folder (OneDrive, Google Drive, a network folder…)", directory: true });
  return typeof r === "string" ? r : null;
}

/** Where to save the sermon transcript (a text file). */
/** The slide holding verse "3:17" (also inside a slide of several verses). */
export function slideForVerse(slides: Slide[], start: string): number {
  const exact = slides.findIndex((s) => s.label === start);
  if (exact >= 0) return exact;
  const [c, v] = start.split(":").map(Number);
  const key = c * 1000 + v;
  return Math.max(0, slides.findIndex((s) => s.first_key <= key && key <= s.last_key));
}

export async function pickTextSave(name: string): Promise<string | null> {
  if (!inTauri) return name;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ title: "Save the sermon transcript", defaultPath: name, filters: [{ name: "Text", extensions: ["txt"] }] })) ?? null;
}

/** Live sermon subtitles: what the speech engine hears, several times a second. */
export async function onCaption(cb: (c: { text: string; final: boolean }) => void): Promise<() => void> {
  if (!inTauri) {
    const ch = new BroadcastChannel("wp-live");
    ch.onmessage = (e) => e.data.type === "caption" && cb(e.data.caption);
    return () => ch.close();
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<{ text: string; final: boolean }>("caption", (e) => cb(e.payload));
}

export async function onIdentify(cb: (role: string) => void): Promise<() => void> {
  if (!inTauri) {
    const ch = new BroadcastChannel("wp-live");
    ch.onmessage = (e) => e.data.type === "identify" && cb(e.data.role);
    return () => ch.close();
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("identify", (e) => cb(e.payload));
}

export async function onSettingsChanged(cb: () => void): Promise<() => void> {
  if (!inTauri) {
    const ch = new BroadcastChannel("wp-live");
    ch.onmessage = (e) => e.data.type === "settings" && cb();
    return () => ch.close();
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen("settings-changed", cb);
}

export async function onLibraryRestored(cb: () => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen("library-restored", cb);
}

/** Native file or folder picker. In the browser preview it returns a sample path. */
export async function pickPath(kind: ImportKind): Promise<string | null> {
  if (!inTauri) return kind === "ew-songs" ? "C:\\EasyWorship\\Songs" : kind === "ewb" ? "C:\\Bibles\\nkjv.ewb" : "C:\\Bibles\\TK2012.bib";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open(
    kind === "ew-songs"
      ? { directory: true, title: "Choose the EasyWorship folder that holds Songs.db and SongWords.db" }
      : { title: "Choose a Bible file", filters: [kind === "ewb" ? { name: "EasyWorship Bible", extensions: ["ewb"] } : { name: "Bible Show Bible", extensions: ["bib"] }] },
  );
  return typeof r === "string" ? r : null;
}

export async function onLive(cb: (b: Broadcast) => void): Promise<() => void> {
  if (inTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<Broadcast>("live-state", (e) => cb(e.payload));
  }
  const ch = new BroadcastChannel("wp-live");
  ch.onmessage = (e) => e.data.type === "live" && cb(e.data.payload);
  const local = (e: Event) => cb((e as CustomEvent<Broadcast>).detail);
  window.addEventListener("wp-local-live", local);
  return () => {
    ch.close();
    window.removeEventListener("wp-local-live", local);
  };
}

export async function onHeartbeat(cb: (revision: number) => void): Promise<() => void> {
  if (inTauri) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<number>("heartbeat", (e) => cb(e.payload));
  }
  const ch = new BroadcastChannel("wp-live");
  ch.onmessage = (e) => e.data.type === "heartbeat" && cb(e.data.payload);
  return () => ch.close();
}

export const isDesktop = inTauri;

