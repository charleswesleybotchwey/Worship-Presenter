// Talks to the Rust core through Tauri. When opened in a plain browser (vite dev),
// a small in-memory stand-in answers the same calls so the screens can be reviewed;
// it shares live state between tabs with BroadcastChannel, so #/output/main in a
// second tab behaves like an output window.

export type Mode = "live" | "blank" | "logo" | "clear";
export type LangMode = "primary" | "second" | "both";

export interface Slide {
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
  | { kind: "announcement"; presentation_id: number; title: string };

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

import { BUILTIN, type ActiveThemes, type Theme, type ThemeKind, type ThemeRow } from "./theme/types";

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

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
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
  outputAck: (role: string, revision: number) => call<void>("output_ack", { role, revision }),
  outputStatus: () => call<OutputStatus[]>("output_status"),
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
  streamStatus: () => call<{ ffmpeg: boolean; running: boolean; seconds: number; file: string | null; destinations: string[]; report: string; installing: string | null }>("stream_status"),
  streamInstallFfmpeg: () => call<void>("stream_install_ffmpeg"),
  streamStart: (req: { record: boolean; folder: string | null; destinations: { kind: string; target: string }[] }) => call<string>("stream_start", { req }),
  streamStop: () => call<string | null>("stream_stop"),
  updateCheck: () => call<{ version: string; notes: string | null } | null>("update_check"),
  updateInstall: () => call<void>("update_install"),
  logError: (window: string, text: string) => call<void>("log_error", { window, text }),
  openLogs: () => call<void>("open_logs"),
  diagnosticReport: () => call<string>("diagnostic_report"),
  bibleCatalogue: () => call<{ id: string; language: string; title: string }[]>("bible_catalogue"),
  bibleDownload: (id: string, code: string, title: string, lang: string) => call<number>("bible_download", { id, code, title, lang }),
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
  pulpitNotes: () => call<PulpitNote[]>("pulpit_notes"),
  pulpitDismiss: (id: number | null) => call<void>("pulpit_dismiss", { id }),
  speechStatus: () => call<SpeechStatus>("speech_status"),
  speechStart: (engine: string, mic: string | null, referencesOnly: boolean) => call<void>("speech_start", { engine, mic, referencesOnly }),
  speechStop: () => call<void>("speech_stop"),
  speechDismiss: (reference: string | null) => call<void>("speech_dismiss", { reference }),
  speechMics: () => call<string[]>("speech_mics"),
  speechInstallVosk: () => call<void>("speech_install_vosk"),
  speechInstallWhisper: (model: string) => call<void>("speech_install_whisper", { model }),
  speechSaveNotes: (path: string) => call<number>("speech_save_notes", { path }),
  speechTestRecording: (path: string) => call<{ transcript: [number, string][]; found: [number, string, string][]; model: string; seconds: number; took_seconds: number }>("speech_test_recording", { path }),
  presentationImport: (path: string) =>
    call<{ stage: "done" | "render_pdf"; method: string; title: string; deck_id: number | null; pdf: string | null; token: string | null; warnings: string[] }>("presentation_import", { path }),
  /** A drawn page as raw bytes (not JSON), so big decks import quickly. */
  presentationPage: async (token: string, index: number, bytes: Uint8Array, ext: "jpg" | "png"): Promise<void> => {
    if (!inTauri) return void mockPage(token, index, bytes, ext);
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

// ---------------------------------------------------------------------------
// Browser stand-in. Real lyrics and verses taken from the Phase 0 import.

const SONGS: { hit: SongHit; sections: { label: string; en: string[]; fat?: string[] }[] }[] = [
  {
    hit: { id: 1, title: "MHB 1, ENG&FANTE", author: "Charles Wesley, 1707-88.", numbers: ["MHB 1"], first_line: "O For a thousand tongues to sing" },
    sections: [
      { label: "V1", en: ["O For a thousand tongues to sing", "My great Redeemer’s praise,", "The glories of my God and King,", "The triumphs of His grace!"], fat: ["Meenya ngyiramá apem matow ndwom", "Meyi mo Pomfò ayɛw,", "Makamfo me Hen N’enyimnyam,", "N’adom no konyimdzi."] },
      { label: "V2", en: ["Jesus! the name that charms our fears,", "That bids our sorrows cease;", "’Tis music in the sinner’s ears,", "’Tis life, and health, and peace."], fat: ["Jesus! dzin a ɔma suro yi,", "Ɔma awerɛhow gyae;", "Ɔyɛ ndwom wɔ ɔbɔnefo aso,", "Ɔyɛ nkwa, ahoɔden, asomdwee."] },
    ],
  },
  {
    hit: { id: 2, title: "MHB 50", author: "Isaac Watts", numbers: ["MHB 50"], first_line: "Come, let us join our cheerful songs" },
    sections: [{ label: "V1", en: ["[G]Come, let us [C]join our [D]cheerful [G]songs", "With [Em]angels [A]round the [D]throne;", "[G]Ten thousand [C]thousand [G]are their [D]tongues,", "But [C]all their [D]joys are [G]one."] }],
  },
];

const VERSES: Record<string, { display: string; v: Record<string, string[]> }> = {
  "jn 3:16": {
    display: "Yohane 3:16",
    v: {
      TK: ["Na sɛdeɛ Onyankopɔn dɔɔ wiase nie, sɛ ɔde ne Ba a ɔwoo no korɔ no maeɛ, na obiara a ɔgye no die no anyera, na wanya daa nkwa."],
      NKJV: ["For God so loved the world that He gave His only begotten Son, that whoever believes in Him should not perish but have everlasting life."],
      GA: ["“Ejaakɛ nɛkɛ Nyɔŋmɔ sumɔ je lɛ, akɛ eŋɔ eBi koome lɛ ehã, koni mɔ fɛɛ mɔ ni heɔ enɔ yeɔ lɛ hiɛ akakpãtã, shi moŋ ena naanɔ wala."],
    },
  },
};

const MOCK_KEYS: Record<number, string> = { 2: "G" };
let mState: LiveState = { revision: 0, item_idx: null, slide_idx: 0, mode: "blank", lang_mode: "both", lines_per_slide: 2, media_playing: true, media_restart: 0, stage_message: null, timer: null, alert: null, auto: null, slide_since_ms: 0, repeat: null, lining_out: false };
const TEMPLATES: Deck[] = [
  { id: null, title: "Offering and tithes", looping: false, seconds: 0, slides: [
    { kind: "text", title: "Offering", body: "“God loves a cheerful giver.” 2 Corinthians 9:7", image: null, seconds: null },
    { kind: "text", title: "Give by Mobile Money", body: "MTN MoMo: 024 000 0000\nName: Your Church Name\nReference: Offering", image: null, seconds: null },
    { kind: "text", title: "Tithes", body: "Bring the whole tithe into the storehouse. Malachi 3:10\nMoMo reference: Tithe and your name", image: null, seconds: null },
    { kind: "text", title: "Bank transfer", body: "Bank: Your Bank, Branch\nAccount name: Your Church Name\nAccount number: 0000000000", image: null, seconds: null } ] },
  { id: null, title: "Birthdays this week", looping: false, seconds: 0, slides: [
    { kind: "text", title: "Happy birthday", body: "Name One\nName Two\nName Three", image: null, seconds: null },
    { kind: "text", title: "Ayɛkoo!", body: "We celebrate you. May God bless your new year.", image: null, seconds: null } ] },
  { id: null, title: "Welcome before service", looping: true, seconds: 12, slides: [
    { kind: "text", title: "Akwaaba! Welcome", body: "We are glad you are here.\nThe service begins shortly.", image: null, seconds: null },
    { kind: "text", title: "Please silence your phone", body: "Thank you.", image: null, seconds: null },
    { kind: "text", title: "This week", body: "Bible study: Wednesday 6:30 pm\nPrayer meeting: Friday 6:30 pm", image: null, seconds: null },
    { kind: "text", title: "Visiting today?", body: "Please stay after the service so we can welcome you.", image: null, seconds: null } ] },
];
let mDecks: Deck[] = [{ ...structuredClone(TEMPLATES[2]), id: 1, seconds: 3 }];
const deckSlides = (d: Deck): Slide[] =>
  d.slides.map((x, i) => ({ label: x.title.trim() || `Slide ${i + 1}`, occurrence: i, first_key: i, last_key: i, primary: x.kind === "text" ? x.body.split("\n").filter((l) => l.trim()) : [], second: [], image: x.kind === "picture" ? x.image : null }));
const deckDurations = (d: Deck) => (d.seconds === 0 && d.slides.every((x) => x.seconds == null) ? [] : d.slides.map((x) => Math.max(2, x.seconds ?? d.seconds) * 1000));
let mBeforeLining = 2;
let mTimerPresets: TimerPreset[] = [
  { id: 1, name: "5-minute countdown", mode: "countdown", value: "5:00" },
  { id: 2, name: "Sermon, 30 minutes", mode: "countdown", value: "30:00" },
  { id: 3, name: "Service starts", mode: "to_time", value: "09:00" },
];
let mAlertPresets: AlertPreset[] = [
  { id: 1, text: "Parent of child number {number}, please come to the children's room.", position: "bottom", style: "band", seconds: 45 },
  { id: 2, text: "The owner of car {registration} is asked to see an usher, please.", position: "bottom", style: "band", seconds: 45 },
  { id: 3, text: "Welcome to our service. Please switch your phone to silent.", position: "bottom", style: "ticker", seconds: 60 },
];
function mParseLength(v: string): number | null {
  const p = v.trim().split(":").map(Number);
  if (p.some(isNaN)) return null;
  const secs = p.length === 1 ? p[0] * 60 : p.length === 2 ? p[0] * 60 + p[1] : p[0] * 3600 + p[1] * 60 + p[2];
  return secs > 0 ? secs * 1000 : null;
}
function mParseClock(v: string): number | null {
  const m = v.trim().toLowerCase().match(/^(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm)?$/);
  if (!m) return null;
  let h = Number(m[1]);
  if (m[3] === "pm" && h < 12) h += 12;
  if (m[3] === "am" && h === 12) h = 0;
  const d = new Date();
  d.setHours(h, Number(m[2] ?? 0), 0, 0);
  return d.getTime();
}
let mMedia: MediaItem[] = [
  { id: 1, kind: "video", path: "/samples/motion-loop.webm", title: "Motion loop blue", duration_ms: null, width: null, height: null, thumb: null, tags: null, bytes: 97_956, missing: false },
  { id: 2, kind: "image", path: "/samples/sunrise.jpg", title: "Sunrise", duration_ms: null, width: null, height: null, thumb: null, tags: null, bytes: 18_510, missing: false },
  { id: 4, kind: "audio", path: "/samples/none.mp3", title: "Instrumental worship 1", duration_ms: 214_000, width: null, height: null, thumb: null, tags: null, bytes: 4_100_000, missing: false },
  { id: 5, kind: "audio", path: "/samples/none2.mp3", title: "Highlife hymn medley", duration_ms: 305_000, width: null, height: null, thumb: null, tags: null, bytes: 6_900_000, missing: false },
  { id: 3, kind: "video", path: "/samples/old-countdown.mp4", title: "Old countdown", duration_ms: 60_000, width: 1920, height: 1080, thumb: null, tags: null, bytes: 0, missing: true },
];
let mItem: ProgramItem | null = null;
let mSlides: Slide[] = [];
let mCaption: string | null = null;
let mPrev: { item: ProgramItem | null; slides: Slide[]; idx: number } | null = null;
let chan: BroadcastChannel | null = null;
const mSchedules: Schedule[] = [
  { id: 1, name: "Sunday first service", service_date: null, template: true, items: [{ kind: "song", song_id: 1, title: "MHB 1, ENG&FANTE" }, { kind: "scripture", reference: "Jn 3:16", versions: ["NKJV", "TK"] }] },
];
let mNextId = 2;
const mAcks = new Map<string, { rev: number; at: number }>();
let mMine: { id: number; name: string; songs: number[] }[] = [];
const mPages = new Map<string, string[]>();
let mPulpit: PulpitNote[] = [];
let mForeign: ForeignDesign[] = [];
let mMidi: { port: string | null; learning: string | null; map: Record<string, string> } = { port: null, learning: null, map: {} };
let mSync: { folder: string | null; role: "share" | "follow" | null; shared_ms: number | null; taken_ms: number | null; waiting: [string, number] | null } = { folder: null, role: null, shared_ms: null, taken_ms: null, waiting: ["MAIN-PC", Date.now() - 600000] };
let mRec: number | null = null;
let mColls: { id: number; kind: "scripture" | "media"; name: string; items: ProgramItem[] }[] = [];
// browser preview: theme rules matched on the sample songs' titles ("MHB 1" is in MHB)
let mRules: ThemeRule[] = [];
const mResolve = (item: ProgramItem | null | undefined): number | null => {
  if (!item) return null;
  for (const r of mRules) {
    if (r.kind === "song" && item.kind === "song" && r.match === "hymnal" && item.title.toUpperCase().startsWith(r.value.toUpperCase() + " ")) return r.theme_id;
    if (r.kind === "scripture" && item.kind === "scripture" && r.match === "version" && item.versions[0] === r.value) return r.theme_id;
  }
  return null;
};
/** Browser preview only: pretend the pastor's phone sent a note. */
(globalThis as unknown as { __wpNote: (t: string) => void }).__wpNote = (text: string) => {
  const title = "Sermon notes (preview)";
  let d = mDecks.find((x) => x.title === title);
  if (!d) mDecks.push((d = { id: Math.max(0, ...mDecks.map((x) => x.id!)) + 1, title, looping: false, seconds: 0, slides: [] }));
  d.slides.push({ kind: "text", title: "", body: text, image: null, seconds: null });
  mPulpit.unshift({ id: Date.now(), device: "Pastor's phone", text, deck_id: d.id!, deck_title: title, slide_idx: d.slides.length - 1 });
};
const mSpeech = { on: null as string | null, caption: "", recent: [] as HeardRef[], ready: false };
function mockPage(token: string, index: number, bytes: Uint8Array, ext: string) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  const list = mPages.get(token) ?? [];
  list[index - 1] = `data:image/${ext === "png" ? "png" : "jpeg"};base64,${btoa(bin)}`;
  mPages.set(token, list);
}
const OUT_DEFAULT = { display_index: null, display_name: null, custom: null, margins: { left: 0, right: 0, top: 0, bottom: 0 }, alpha: false, song_theme_id: null, scripture_theme_id: null, show_on_startup: false };
let mOutputs: OutputConfig[] = [
  { ...OUT_DEFAULT, role: "main", name: "Main", on_logo: "logo", on_blank: "blank" },
  { ...OUT_DEFAULT, role: "alternate", name: "Lower third", on_logo: "transparent", on_blank: "transparent" },
  { ...OUT_DEFAULT, role: "foldback", name: "Stage", on_logo: "blank", on_blank: "blank" },
];
let mIntervals: ServiceInterval[] = [];
const mMusic = { started: false, playing: false, paused: false, index: 0, pos: 0, at: Date.now(), volume: 0.8, ducked: false, ids: [] as number[], rules: { on_live: "fade_out", with_announcements: true, crossfade_ms: 4000, looping: true } as MusicRules };
const mHeld = new Set<string>();
const mSettings: Record<string, string> = { project_on_restart: "false", low_spec_mode: "false", lines_per_slide: "2" };
let mThemeRev = 1;
let mThemes: ThemeRow[] = (["song", "scripture", "lower_third"] as const).map((k, i) => ({ id: i + 1, name: BUILTIN[k].name, kind: k, is_default: true, theme: structuredClone(BUILTIN[k]) }));
const mThemeFor = (k: ThemeKind) => (mThemes.find((t) => t.kind === k && t.is_default) ?? mThemes.find((t) => t.kind === k))!.theme;
function themesChanged() {
  mThemeRev++;
  chan ??= new BroadcastChannel("wp-live");
  chan.postMessage({ type: "themes", payload: { rev: mThemeRev, themes: mThemes } });
  emit();
}
let mProfiles: Profile[] = [
  { slug: "default", name: "Main", active: true, songs: 4531, bibles: 20 },
  { slug: "youth-service", name: "Youth Service", active: false, songs: 120, bibles: 2 },
];
const mBackups: BackupInfo[] = [
  { path: "C:\\…\\library-20261101-090000-weekly.db", file: "library-20261101-090000-weekly.db", taken: "2026-11-01 09:00", reason: "weekly", bytes: 40_900_000 },
];

function parseBody(body: string) {
  const out: { label: string; lines: string[] }[] = [];
  for (const raw of body.split("\n")) {
    const t = raw.trim();
    if (!t) continue;
    const m = t.match(/^\[(.+)\]$/);
    if (m) {
      const l = m[1].toLowerCase();
      const num = l.replace(/\D/g, "");
      out.push({ label: l.startsWith("verse") ? `V${num}` : l.startsWith("chorus") ? "C" : l.startsWith("bridge") ? "B" : m[1].toUpperCase(), lines: [] });
    } else {
      if (!out.length) out.push({ label: "V1", lines: [] });
      out[out.length - 1].lines.push(t);
    }
  }
  return out.filter((x) => x.lines.length);
}

function mockPreview(d: SongDoc, lines: number): Slide[] {
  const main = parseBody(d.lyrics[0]?.body ?? "");
  if (!main.length) throw new Error("A song needs at least one line of lyrics.");
  const second = d.lyrics[1] ? parseBody(d.lyrics[1].body) : [];
  const seq = d.sequence.split(/[\s,]+/).filter(Boolean);
  for (const l of seq) if (!main.some((m) => m.label === l)) throw new Error(`The play order uses ${l}, but there is no [${l}] section.`);
  const order = seq.length ? seq.map((l) => main.find((m) => m.label === l)!) : main;
  const lps = d.lines_per_slide ?? lines;
  const out: Slide[] = [];
  order.forEach((sec, occ) => {
    const sl = second.find((x) => x.label === sec.label)?.lines ?? [];
    for (let i = 0; i < sec.lines.length; i += lps)
      out.push({ label: sec.label, occurrence: occ, first_key: i, last_key: i + lps - 1, primary: sec.lines.slice(i, i + lps), second: sl.slice(Math.floor((i * sl.length) / sec.lines.length), Math.floor(((i + lps) * sl.length) / sec.lines.length)) });
  });
  return out;
}

function splitSong(id: number, lines: number, mode: LangMode): Slide[] {
  const s = SONGS.find((x) => x.hit.id === id);
  if (!s) return [];
  const out: Slide[] = [];
  s.sections.forEach((sec, occ) => {
    for (let i = 0; i < sec.en.length; i += lines) {
      out.push({
        label: sec.label,
        occurrence: occ,
        first_key: i,
        last_key: Math.min(i + lines, sec.en.length) - 1,
        primary: (mode === "second" && sec.fat ? [] : sec.en.slice(i, i + lines)).map((l) => l.replace(/\[[A-G][^\]]*\]/g, "")),
        chords: sec.en.some((l) => l.includes("[")) ? sec.en.slice(i, i + lines) : undefined,
        second: mode === "primary" || !sec.fat ? [] : sec.fat.slice(i, i + lines),
      });
    }
  });
  return out;
}

function scripture(ref: string, versions: string[]): [Slide[], string | null] {
  const key = ref.toLowerCase().replace(/john|yohane|yɔhane/, "jn").replace(/\s+(\d)/, " $1").trim();
  const hit = VERSES[key];
  if (!hit) throw new Error(`No verses found for "${ref}" in the sample data. Try Jn 3:16.`);
  const [first, ...rest] = versions.length ? versions : ["NKJV"];
  return [
    (hit.v[first] ?? []).map((t, i) => ({ label: "3:16", occurrence: 0, first_key: i, last_key: i, primary: [t], second: rest.flatMap((r) => hit.v[r] ?? []) })),
    hit.display,
  ];
}

function mediaOf(it: ProgramItem | null): MediaItem | null {
  if (it?.kind !== "media") return null;
  return mMedia.find((m) => m.id === it.media_id) ?? null;
}

function countRepeat(idx: number) {
  const r = mState.repeat;
  if (!r) return null;
  const nw = mSlides[idx], old = mSlides[mState.slide_idx], prev = mSlides[idx - 1];
  const entering = nw?.label === r.label && (!old || old.label !== r.label || old.occurrence !== nw.occurrence || idx < mState.slide_idx);
  const first = idx === 0 || !prev || prev.label !== nw?.label || prev.occurrence !== nw?.occurrence;
  return entering && first ? { ...r, done: r.done + 1 } : r;
}

function emit() {
  chan ??= new BroadcastChannel("wp-live");
  const payload: Broadcast = { state: mState, item: mItem, slides: mSlides, caption: mCaption, theme_rev: mThemeRev, media: mediaOf(mItem), held: [...mHeld], song_key: mItem?.kind === "song" ? MOCK_KEYS[mItem.song_id] ?? null : null, rule_theme_id: mResolve(mItem), song_copyright: mItem?.kind === "song" ? "Public Domain" : null };
  chan.postMessage({ type: "live", payload });
  window.dispatchEvent(new CustomEvent("wp-local-live", { detail: payload }));
}

if (!inTauri && typeof window !== "undefined") {
  chan = new BroadcastChannel("wp-live");
  // the tab that holds a live item answers resync requests from output tabs
  chan.addEventListener("message", (e) => e.data.type === "resync" && mItem && emit());
  chan.addEventListener("message", (e) => e.data.type === "settings" && e.data.payload && Object.assign(mSettings, e.data.payload));
  // keep every tab's sample themes in step, like the shared core does in the app
  chan.addEventListener("message", (e) => {
    if (e.data.type === "themes-request" && mThemeRev > 1) chan?.postMessage({ type: "themes", payload: { rev: mThemeRev, themes: mThemes } });
    if (e.data.type !== "themes" || e.data.payload.rev <= mThemeRev) return;
    mThemes = e.data.payload.themes;
    mThemeRev = e.data.payload.rev;
  });
  chan.addEventListener("message", (e) => {
    if (e.data.type === "output-config" && e.data.configs) mOutputs = e.data.configs;
  });
  chan.addEventListener("message", (e) => {
    if (e.data.type === "ack") mAcks.set(e.data.role, { rev: e.data.revision, at: Date.now() });
    if (e.data.type === "ack-close") mAcks.delete(e.data.role);
  });
  // a tab opened later asks the others for themes edited before it existed
  chan.postMessage({ type: "themes-request" });
  setInterval(() => {
    chan?.postMessage({ type: "heartbeat", payload: mState.revision });
    const au = mState.auto;
    if (!mItem || !au || au.paused || mState.mode !== "live") return;
    const d = au.durations_ms[mState.slide_idx];
    if (d == null || Date.now() - mState.slide_since_ms < d) return;
    const next = mState.slide_idx + 1 < mSlides.length ? mState.slide_idx + 1 : au.looping ? 0 : null;
    if (next != null) bump({ slide_idx: next, slide_since_ms: Date.now() });
  }, 500);
}

function bump(p: Partial<LiveState>) {
  mState = { ...mState, ...p, revision: mState.revision + 1 };
  emit();
  return mState;
}

async function mock(cmd: string, a: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "search_songs": {
      const q = String(a.query).toLowerCase().trim();
      return SONGS.map((s) => s.hit).filter((h) => !q || `${h.title} ${h.first_line} ${h.numbers.join(" ")}`.toLowerCase().includes(q));
    }
    case "preview_song":
      return splitSong(a.songId as number, mState.lines_per_slide, mState.lang_mode);
    case "bible_versions":
      return [
        { id: 1, code: "NKJV", name: "New King James Version", lang: "en" },
        { id: 2, code: "TK", name: "Twerɛ Kronkron (2012)", lang: "tw" },
        { id: 3, code: "GA", name: "Ŋmalɛ Krɔŋkrɔŋ Lɛ (2020)", lang: "gaa" },
      ];
    case "preview_scripture":
      return scripture(String(a.reference), a.versions as string[]);
    case "go_live": {
      {
        const k = (a.item as ProgramItem).kind;
        if (k === "announcement" && mMusic.rules.with_announcements && mMusic.ids.length) Object.assign(mMusic, { started: true, playing: true, paused: false, ducked: false, at: Date.now() });
        else if (mMusic.playing && k !== "announcement") {
          if (mMusic.rules.on_live === "fade_out") mMusic.playing = false;
          if (mMusic.rules.on_live === "duck") mMusic.ducked = true;
        }
      }
      mPrev = { item: mItem, slides: mSlides, idx: mState.slide_idx };
      const item = a.item as ProgramItem;
      if (item.kind === "song" && item.lyrics?.length) {
        mSlides = mockPreview({ id: item.song_id, title: item.title, author: null, copyright: null, ccli: null, numbers: [], lyrics: item.lyrics, sequence: "", lines_per_slide: null }, mState.lines_per_slide);
        mCaption = item.title;
      } else if (item.kind === "song") {
        mSlides = splitSong(item.song_id, mState.lines_per_slide, mState.lang_mode);
        mCaption = item.title;
      } else if (item.kind === "media") {
        mSlides = [{ label: "Media", occurrence: 0, first_key: 0, last_key: 0, primary: [], second: [] }];
        mCaption = item.title;
      } else if (item.kind === "announcement") {
        const d = mDecks.find((x) => x.id === item.presentation_id)!;
        mSlides = deckSlides(d);
        mCaption = item.title;
        const durations_ms = deckDurations(d);
        mItem = item;
        return bump({ item_idx: 0, slide_idx: Math.min(a.slideIdx as number, mSlides.length - 1), mode: "live", slide_since_ms: Date.now(), repeat: null, auto: durations_ms.length ? { durations_ms, looping: d.looping, paused: false } : null });
      } else {
        [mSlides, mCaption] = scripture(item.reference, item.versions);
      }
      mItem = item;
      return bump({ item_idx: 0, slide_idx: Math.min(a.slideIdx as number, mSlides.length - 1), mode: "live", media_playing: true, media_restart: mState.media_restart + 1, slide_since_ms: Date.now(), auto: null, repeat: null });
    }
    case "show_slide": {
      if (!mSlides.length) throw new Error("Nothing is live. Send an item to Live first.");
      const idx = Math.max(0, Math.min(mSlides.length - 1, a.slideIdx != null ? (a.slideIdx as number) : mState.slide_idx + ((a.delta as number) ?? 0)));
      return bump({ slide_idx: idx, slide_since_ms: Date.now(), repeat: countRepeat(idx) });
    }
    case "set_mode":
      // Blank, Logo and Clear latch; "live" is Resume output
      return bump({ mode: a.mode as Mode });
    case "set_layout": {
      const cur = mSlides[mState.slide_idx];
      const lines = (a.lines as number) ?? mState.lines_per_slide;
      const lang = (a.langMode as LangMode) ?? mState.lang_mode;
      let idx = mState.slide_idx;
      if (mItem?.kind === "song") {
        mSlides = splitSong(mItem.song_id, lines, lang);
        if (cur) idx = Math.max(0, mSlides.findIndex((s) => s.occurrence === cur.occurrence && s.first_key <= cur.first_key && cur.first_key <= s.last_key));
      }
      return bump({ lines_per_slide: lines, lang_mode: lang, slide_idx: idx });
    }
    case "undo_go_live":
      if (mPrev) {
        mItem = mPrev.item;
        mSlides = mPrev.slides;
        const idx = mPrev.idx;
        mPrev = null;
        return bump({ slide_idx: idx });
      }
      return bump({});
    case "resync":
      chan?.postMessage({ type: "resync" });
      return;
    case "list_displays":
      return [
        { index: 0, name: "Built-in display", width: 1920, height: 1080, primary: true },
        { index: 1, name: "Projector (HDMI)", width: 1920, height: 1080, primary: false },
      ];
    case "open_output":
      window.open(`${location.pathname}#/output/${a.role}`, `wp-${a.role}`, "width=960,height=540");
      return;
    case "close_output":
      mAcks.delete(String(a.role));
      chan?.postMessage({ type: "ack-close", role: a.role });
      return;
    case "output_ack":
      chan ??= new BroadcastChannel("wp-live");
      chan.postMessage({ type: "ack", role: a.role, revision: a.revision });
      return;
    case "release_output":
      mHeld.delete(String(a.role));
      emit();
      return;
    case "output_status":
      return [...mAcks.entries()].map(([role, v]) => ({ role, display_index: null, revision: v.rev, silent_secs: (Date.now() - v.at) / 1000 }));
    case "list_schedules":
      return mSchedules.map((x) => ({ id: x.id, name: x.name, service_date: x.service_date, template: x.template, items: x.items.length }));
    case "open_schedule": {
      const x = mSchedules.find((y) => y.id === a.id);
      if (!x) throw new Error("That service order no longer exists.");
      return structuredClone(x);
    }
    case "new_schedule": {
      const t = mSchedules.find((y) => y.id === a.fromTemplate);
      const x: Schedule = { id: mNextId++, name: String(a.name), service_date: (a.serviceDate as string) ?? null, template: !!a.template, items: t ? structuredClone(t.items) : [] };
      mSchedules.push(x);
      return structuredClone(x);
    }
    case "save_schedule": {
      const x = mSchedules.find((y) => y.id === a.id);
      if (x) x.items = structuredClone(a.items as ProgramItem[]);
      return;
    }
    case "delete_schedule":
      mSchedules.splice(mSchedules.findIndex((y) => y.id === a.id), 1);
      return;
    case "last_schedule":
      return null;
    case "get_song": {
      const x = SONGS.find((y) => y.hit.id === a.id);
      if (!x) throw new Error("That song is no longer in the library.");
      const body = (lang: "en" | "fat") => x.sections.filter((sec) => sec[lang]).map((sec) => `[${sec.label}]\n${sec[lang]!.join("\n")}`).join("\n\n");
      const lyrics = [{ lang: "en", body: body("en") }];
      if (x.sections.some((sec) => sec.fat)) lyrics.push({ lang: "fat", body: body("fat") });
      return { id: x.hit.id, title: x.hit.title, author: x.hit.author, copyright: "Public Domain", ccli: null, numbers: x.hit.numbers.map((n) => ({ hymnal: n.split(" ")[0], number: n.split(" ")[1] })), lyrics, sequence: "", lines_per_slide: null } satisfies SongDoc;
    }
    case "preview_song_doc":
      return mockPreview(a.doc as SongDoc, mState.lines_per_slide);
    case "refresh_live_item": {
      if (mItem?.kind === "song") {
        if (a.title) mItem = { ...mItem, title: String(a.title) };
        mSlides = splitSong(mItem.song_id, mState.lines_per_slide, mState.lang_mode);
      }
      return bump({});
    }
    case "song_files_review":
      await new Promise((r) => setTimeout(r, 300));
      return { name: "Song files", read: 3, recognised: 2, ambiguous: [], rejected: [{ item: "notes.txt", reason: "the file is empty" }], duplicates: [], notes: ["Formats found: SongSelect, OpenLyrics."] };
    case "song_files_import":
      return { name: "Song files", read: 3, imported: 2, skipped: 1, fixed_chars: 0, notes: [] };
    case "import_review":
      await new Promise((r) => setTimeout(r, 400));
      return a.kind === "ew-songs"
        ? { name: "Songs", read: 4531, recognised: 4410, ambiguous: [{ item: "NCAN 12", reason: "3× U+03B5 → \"ɛ\" (Greek epsilon used for open e)" }, { item: "GMHB 40", reason: "2× U+1D10 → \"ɔ\" (small capital open o used for open o)" }], rejected: [], duplicates: ["MHB 1", "MHB 50"], notes: [] }
        : { name: "Twerɛ Kronkron (2012)", read: 35477, recognised: 35477, ambiguous: [], rejected: [{ item: "Ɛksodɔs 1:21", reason: "empty verse" }], duplicates: a.code ? [] : ["TK"], notes: ["11752 footnotes kept out (footnotes are not projected yet)"] };
    case "relink_media": {
      mMedia = mMedia.map((m) => (m.id === a.id ? { ...m, missing: false, thumb: null } : m));
      return mMedia.find((m) => m.id === a.id);
    }
    case "identify_monitors":
      return 1;
    case "identify_output":
      chan?.postMessage({ type: "identify", role: a.role });
      return;
    case "get_settings":
      return { ...mSettings };
    case "set_setting":
      mSettings[String(a.key)] = String(a.value);
      // other windows of the browser preview keep their own copy: send it along
      chan?.postMessage({ type: "settings", payload: { ...mSettings } });
      return;
    case "save_song": {
      const d = a.doc as SongDoc;
      if (!d.title.trim()) throw new Error("A song needs a title.");
      mockPreview(d, 2);
      const parse = (lang: string) => parseBody(d.lyrics.find((l) => l.lang === lang)?.body ?? "");
      const en = parse(d.lyrics[0].lang);
      const fat = d.lyrics[1] ? parse(d.lyrics[1].lang) : [];
      const sections = en.map((sec) => ({ label: sec.label, en: sec.lines, fat: fat.find((f) => f.label === sec.label)?.lines }));
      const hit: SongHit = { id: d.id ?? 100 + SONGS.length, title: d.title.trim(), author: d.author, numbers: d.numbers.map((n) => `${n.hymnal.toUpperCase()} ${n.number}`), first_line: sections[0]?.en[0] ?? null };
      const i = SONGS.findIndex((y) => y.hit.id === hit.id);
      if (i >= 0) SONGS[i] = { hit, sections };
      else SONGS.push({ hit, sections });
      return { id: hit.id, live: mItem?.kind === "song" && mItem.song_id === hit.id && !mItem.lyrics };
    }
    case "delete_song":
      if (mItem?.kind === "song" && mItem.song_id === a.id) throw new Error("This song is on the screens. Send something else live first, then delete it.");
      SONGS.splice(SONGS.findIndex((y) => y.hit.id === a.id), 1);
      return;
    case "active_themes":
      return { rev: mThemeRev, song: mThemeFor("song"), scripture: mThemeFor("scripture"), lower_third: mThemeFor("lower_third"), announcement: mThemes.find((t) => t.kind === "announcement")?.theme ?? BUILTIN.announcement };
    case "list_themes":
      return structuredClone(mThemes);
    case "save_theme": {
      const t = a.theme as Theme;
      if (!t.name.trim()) throw new Error("A theme needs a name.");
      let id = a.id as number | null;
      if (id == null) {
        id = Math.max(0, ...mThemes.map((x) => x.id)) + 1;
        mThemes.push({ id, name: t.name, kind: t.kind, is_default: false, theme: structuredClone(t) });
      } else mThemes = mThemes.map((x) => (x.id === id ? { ...x, name: t.name, kind: t.kind, theme: structuredClone(t) } : x));
      themesChanged();
      return id;
    }
    case "duplicate_theme": {
      const src = mThemes.find((x) => x.id === a.id)!;
      const id = Math.max(...mThemes.map((x) => x.id)) + 1;
      mThemes.push({ ...structuredClone(src), id, name: `${src.name} copy`, is_default: false, theme: { ...structuredClone(src.theme), name: `${src.name} copy` } });
      return id;
    }
    case "delete_theme": {
      const t = mThemes.find((x) => x.id === a.id)!;
      if (mThemes.filter((x) => x.kind === t.kind).length < 2) throw new Error("This is the only theme of its type. Make another one before deleting it.");
      mThemes = mThemes.filter((x) => x.id !== a.id);
      if (t.is_default) mThemes.find((x) => x.kind === t.kind)!.is_default = true;
      themesChanged();
      return;
    }
    case "set_default_theme":
      mThemes = mThemes.map((x) => (x.kind === a.kind ? { ...x, is_default: x.id === a.id } : x));
      themesChanged();
      return;
    case "list_profiles":
      return structuredClone(mProfiles);
    case "foreign_theme_review": {
      // the browser preview reads a review made from real theme files (test data)
      const r = await fetch("/samples/theme-review.json").catch(() => null);
      if (!r || !r.ok) throw new Error("No sample theme review in the browser preview.");
      mForeign = (await r.json()) as ForeignDesign[];
      return mForeign;
    }
    case "foreign_theme_import": {
      const out = (a.chosen as number[]).map((i) => {
        const d = mForeign[i];
        const id = Math.max(0, ...mThemes.map((t) => t.id)) + 1;
        mThemes.push({ id, name: d.name, kind: d.kind, is_default: false, theme: d.preview as never });
        return { id, name: d.name };
      });
      return out;
    }
    case "set_text_adjust":
      return bump({ text_scale: Math.max(0.6, Math.min(1.6, a.scale as number)), text_shift: Math.max(-30, Math.min(30, a.shift as number)) });
    case "set_transpose":
      return bump({ transpose: Math.max(-11, Math.min(11, a.semitones as number)) });
    case "set_camera":
      return bump({ camera: (a.camera as CameraLive) ?? null });
    case "set_subtitles":
      return bump({ subtitles: (a.target as LiveState["subtitles"]) ?? null });
    case "pulpit_notes":
      return structuredClone(mPulpit);
    case "pulpit_dismiss":
      mPulpit = a.id == null ? [] : mPulpit.filter((n) => n.id !== a.id);
      return;
    case "speech_status":
      return { listening: mSpeech.on, caption: mSpeech.caption, recent: mSpeech.recent, problem: null, vosk_ready: mSpeech.ready, installing: null, windows_speech: true };
    case "speech_start": {
      if (a.engine === "vosk" && !mSpeech.ready) throw new Error("The Vosk engine is not downloaded yet. Settings → Scripture recognition → Download.");
      mSpeech.on = a.engine === "vosk" ? "Vosk" : "Windows Speech";
      // the browser preview pretends to hear a sermon
      const lines = ["so turn with me to John three sixteen", "and also first Corinthians thirteen verses four to seven", "Psalm twenty three"];
      const say = (text: string, final: boolean) => mState.subtitles && (chan ??= new BroadcastChannel("wp-live")).postMessage({ type: "caption", caption: { text, final } });
      lines.forEach((l, i) =>
        setTimeout(() => {
          if (!mSpeech.on) return;
          say(l.split(" ").slice(0, 4).join(" "), false);
          setTimeout(() => say(l, true), 400);
          mSpeech.caption = l;
          const ref = ["John 3:16", "1 Corinthians 13:4-7", "Psalms 23"][i];
          mSpeech.recent.unshift({ reference: ref, book_no: 0, chapter: 0, verse_from: null, verse_to: null, heard: l, at: Date.now() / 1000 });
        }, 1500 + i * 4000),
      );
      return;
    }
    case "speech_stop":
      mSpeech.on = null;
      mSpeech.caption = "";
      return;
    case "speech_dismiss":
      mSpeech.recent = a.reference ? mSpeech.recent.filter((r) => r.reference !== a.reference) : [];
      return;
    case "speech_mics":
      return ["Microphone (Realtek Audio)", "Line In (Behringer UMC202HD)"];
    case "speech_install_vosk":
      await new Promise((r) => setTimeout(r, 800));
      mSpeech.ready = true;
      return;
    case "list_fonts":
      return ["Arial", "Bahnschrift", "Calibri", "Cambria", "Georgia", "Inter", "Noto Sans", "Noto Serif", "Segoe UI", "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana"].map((name) => ({ name, ghanaian: name.startsWith("Noto") || name === "Segoe UI", styles: 4 }));
    case "presentation_import":
      // the browser preview behaves like a PC with LibreOffice: the PDF is drawn here
      return { stage: "render_pdf", method: "libreoffice", title: String(a.path).replace(/\.\w+$/, ""), deck_id: null, pdf: "/samples/harvest.pdf", token: "abc123", warnings: [] };
    case "presentation_finish": {
      const pages = mPages.get(String(a.token)) ?? [];
      const id = Math.max(0, ...mDecks.map((x) => x.id!)) + 1;
      mDecks.push({ id, title: String(a.title), looping: false, seconds: 0, slides: pages.map((img, i) => ({ kind: "picture", title: `Slide ${i + 1}`, body: "", image: img, seconds: null })) });
      mPages.delete(String(a.token));
      return id;
    }
    case "output_configs":
      return structuredClone(mOutputs);
    case "output_config":
      return structuredClone(mOutputs.find((o) => o.role === a.role) ?? mOutputs[0]);
    case "save_output_config": {
      const c = a.config as OutputConfig;
      if (c.custom && (c.custom.w < 64 || c.custom.h < 64)) throw new Error("The output must be at least 64 × 64 pixels.");
      mOutputs = mOutputs.map((o) => (o.role === c.role ? structuredClone(c) : o));
      (chan ??= new BroadcastChannel("wp-live")).postMessage({ type: "output-config", role: c.role, configs: mOutputs });
      return;
    }
    case "service_intervals":
      return structuredClone(mIntervals);
    case "save_service_interval": {
      const i = a.interval as ServiceInterval;
      if (!i.name.trim()) throw new Error("Give the service a name, e.g. First service.");
      if (!(i.start < i.end)) throw new Error("The service must end after it starts.");
      const id = i.id ?? Math.max(0, ...mIntervals.map((x) => x.id!)) + 1;
      mIntervals = [...mIntervals.filter((x) => x.id !== id), { ...i, id }];
      return id;
    }
    case "delete_service_interval":
      mIntervals = mIntervals.filter((x) => x.id !== a.id);
      return;
    case "current_service_interval":
      return null;
    case "usage_report":
      return [{ title: "MHB 1, ENG&FANTE", numbers: "MHB 1", author: "Charles Wesley", copyright: "Public Domain", ccli: "", interval: "First service", uses: 2, first: "2026-11-01", last: "2026-11-08" }];
    case "save_usage_report":
      return 1;
    case "song_tree":
      return {
        all: SONGS.length,
        collections: [{ key: "hymnal:MHB", name: "MHB (English)", count: SONGS.length }],
        mine: mMine.map((c) => ({ key: `mine:${c.id}`, name: c.name, count: c.songs.length })),
      };
    case "browse_songs": {
      const sc = a.scope as SongScope;
      const q = String(a.query).toLowerCase().trim();
      let list = SONGS.map((x) => x.hit);
      if (sc.kind === "mine") list = (mMine.find((c) => c.id === sc.id)?.songs ?? []).map((id) => SONGS.find((x) => x.hit.id === id)!.hit);
      if (q) list = list.filter((h) => `${h.title} ${h.numbers.join(" ")} ${h.first_line} ${h.author}`.toLowerCase().includes(q));
      return list.map((h) => ({ id: h.id, number: h.numbers[0] ?? null, title: h.title, first_line: h.first_line, author: h.author, copyright: "Public Domain", langs: h.title.includes("FANTE") ? ["en", "fat"] : ["en"], sort_key: (h.numbers[0] ?? "").replace(/\d+/g, (d) => d.padStart(8, "0")) }));
    }
    case "collection_create": {
      const name = String(a.name).trim();
      if (!name) throw new Error("A collection needs a name.");
      if (mMine.some((c) => c.name === name)) throw new Error(`There is already a collection called “${name}”.`);
      const id = Math.max(0, ...mMine.map((c) => c.id)) + 1;
      mMine.push({ id, name, songs: [] });
      return id;
    }
    case "collection_rename":
      mMine = mMine.map((c) => (c.id === a.id ? { ...c, name: String(a.name) } : c));
      return;
    case "collection_delete":
      mMine = mMine.filter((c) => c.id !== a.id);
      return;
    case "collection_add": {
      const c = mMine.find((x) => x.id === a.id)!;
      const add = (a.songIds as number[]).filter((x) => !c.songs.includes(x));
      c.songs.push(...add);
      return add.length;
    }
    case "collection_remove": {
      const c = mMine.find((x) => x.id === a.id)!;
      c.songs = c.songs.filter((x) => !(a.songIds as number[]).includes(x));
      return;
    }
    case "browse_chapter": {
      const typed = String(a.typed).trim();
      // like the app: words that are not a reference are not a chapter
      if (!/\d/.test(typed) && !/^(john|jn|yohane)$/i.test(typed)) throw new Error(`“${typed}” is not a Bible reference.`);
      const m = typed.match(/(\d+)(?::(\d+)(?:-(\d+))?)?\s*$/);
      const from = m?.[2] ? Number(m[2]) : 0, to = m?.[3] ? Number(m[3]) : from;
      const v16 = VERSES["jn 3:16"].v[String(a.version)]?.[0] ?? "For God so loved the world…";
      return {
        version: a.version, book: "John", book_no: 43, chapter: 3, chapters: 21,
        rows: Array.from({ length: 21 }, (_, i) => ({ verse: i + 1, reference: `John 3:${i + 1}`, text: i + 1 === 16 ? v16 : `(sample text for John 3:${i + 1} in the browser preview)`, heading: null, selected: from > 0 && i + 1 >= from && i + 1 <= to })),
      };
    }
    case "search_bible": {
      const q = String(a.query).toLowerCase();
      const all = [
        { version: "NKJV", reference: "John 3:16", book_no: 43, chapter: 3, verse: 16, text: "For God so loved the world that He gave His only begotten Son, that whoever believes in Him should not perish but have everlasting life." },
        { version: "NKJV", reference: "Psalm 23:1", book_no: 19, chapter: 23, verse: 1, text: "The LORD is my shepherd; I shall not want." },
        { version: "NKJV", reference: "John 4:10", book_no: 43, chapter: 4, verse: 10, text: "Jesus answered and said to her, “If you knew the gift of God, and who it is who says to you, ‘Give Me a drink,’ you would have asked Him, and He would have given you living water.”" },
      ];
      return all.filter((h) => q.replace(/"/g, "").split(/\s+/).every((w) => h.text.toLowerCase().includes(w)));
    }
    case "rebuild_search":
      return SONGS.length;
    case "reset_song_looks":
      return 0;
    case "music_status": {
      const now = Date.now();
      if (mMusic.playing && !mMusic.paused) mMusic.pos += now - mMusic.at;
      mMusic.at = now;
      const items = mMusic.ids.map((id) => mMedia.find((m) => m.id === id)).filter(Boolean) as MediaItem[];
      if (mMusic.pos > 180_000) (mMusic.index = (mMusic.index + 1) % Math.max(1, items.length)), (mMusic.pos = 0);
      return { started: mMusic.started, status: { playing: mMusic.playing, paused: mMusic.paused, title: mMusic.playing ? items[mMusic.index]?.title ?? null : null, index: mMusic.playing ? mMusic.index : null, position_ms: mMusic.pos, length_ms: 180_000, volume: mMusic.volume, ducked: mMusic.ducked, problem: null }, rules: mMusic.rules, playlist_ids: mMusic.ids };
    }
    case "music_set_playlist":
      mMusic.ids = a.mediaIds as number[];
      return;
    case "music_play":
      if (!mMusic.ids.length) throw new Error("Add some music to the playlist first.");
      Object.assign(mMusic, { started: true, playing: true, paused: false, at: Date.now() });
      if (a.index != null) (mMusic.index = a.index as number), (mMusic.pos = 0);
      return;
    case "music_pause":
      mMusic.paused = !!a.paused;
      return;
    case "music_next":
    case "music_previous":
      mMusic.index = Math.max(0, (mMusic.index + (cmd === "music_next" ? 1 : -1)) % Math.max(1, mMusic.ids.length));
      mMusic.pos = 0;
      return;
    case "music_stop":
      mMusic.playing = false;
      return;
    case "music_volume":
      mMusic.volume = a.volume as number;
      return;
    case "music_unduck":
      mMusic.ducked = false;
      return;
    case "music_set_rules":
      mMusic.rules = a.rules as MusicRules;
      return;
    case "remote_status":
      return { running: false, url: null, stage_url: null, pin: null, qr_svg: null, lan_ip: "192.168.1.10", sessions: [], log: [] };
    case "remote_start":
      throw new Error("The phone remote runs in the desktop app only. To try the pages: cargo run -p wp-remote --example demo");
    case "remote_stop":
    case "remote_revoke":
    case "remote_end_all":
      return;
    case "remote_new_pin":
      return "000000";
    case "ndi_status":
      return { runtime: null, problem: "NDI works in the desktop app only. In this browser preview nothing is sent.", capture_supported: false, sources: [] };
    case "ndi_test_pattern":
    case "ndi_start":
      throw new Error("NDI works in the desktop app only. In this browser preview nothing is sent.");
    case "ndi_sources":
      return [{ name: "MEDIA-PC (OBS Program)", url: "192.168.1.20:5961" }, { name: "PTZ-CAM-1 (Stage)", url: "192.168.1.31:5961" }];
    case "ndi_audio":
    case "theme_rules":
      return mRules;
    case "sync_status":
      return { ...mSync, computer: "MEDIA-LAPTOP" };
    case "sync_set":
      mSync.folder = (a.folder as string) ?? null;
      mSync.role = (a.role as "share" | "follow") ?? null;
      return;
    case "sync_share_now":
      mSync.shared_ms = Date.now();
      return 3;
    case "sync_take_now":
      mSync.taken_ms = Date.now();
      mSync.waiting = null;
      return "MAIN-PC";
    case "stream_status":
      return { ffmpeg: true, running: mRec !== null, seconds: mRec ? Math.floor((Date.now() - mRec) / 1000) : 0, file: null, destinations: [], report: mRec ? "frame=  930 fps= 30 bitrate=4421.6kbits/s" : "", installing: null };
    case "stream_install_ffmpeg":
      return;
    case "stream_start":
      mRec = Date.now();
      return "Recording to C:\\Users\\Media\\Videos\\Worship Presenter\\Service 2026-11-15 0930.mp4";
    case "stream_stop":
      mRec = null;
      return "C:\\Users\\Media\\Videos\\Worship Presenter\\Service 2026-11-15 0930.mp4";
    case "update_check":
      return { version: "0.2.0", notes: "Recording, going live online, crash log." };
    case "update_install":
      return;
    case "log_error":
    case "open_logs":
      return;
    case "diagnostic_report":
      return "Worship Presenter 0.1.0\nSystem: browser preview\n\nRecent log:\n(none)";
    case "bible_catalogue":
      return [
        { id: "engwebp", language: "English", title: "World English Bible" },
        { id: "engkjv", language: "English", title: "King James Version" },
        { id: "eweXYZ", language: "Ewe", title: "Ewe Bible (sample)" },
      ];
    case "bible_download":
      await new Promise((ok) => setTimeout(ok, 300));
      return 31102;
    case "item_collections":
      return mColls.filter((c) => c.kind === a.kind).map((c) => ({ id: c.id, kind: c.kind, name: c.name, count: c.items.length }));
    case "item_collection_create": {
      const id = mColls.length + 1;
      mColls.push({ id, kind: a.kind as "scripture" | "media", name: String(a.name), items: [] });
      return id;
    }
    case "item_collection_rename":
      mColls.forEach((c) => c.id === a.id && (c.name = String(a.name)));
      return;
    case "item_collection_delete":
      mColls = mColls.filter((c) => c.id !== a.id);
      return;
    case "item_collection_add": {
      const c = mColls.find((x) => x.id === a.id);
      if (c && !c.items.some((i) => JSON.stringify(i) === JSON.stringify(a.item))) c.items.push(a.item as ProgramItem);
      return;
    }
    case "item_collection_remove": {
      const c = mColls.find((x) => x.id === a.id);
      if (c) c.items = c.items.filter((i) => JSON.stringify(i) !== JSON.stringify(a.item));
      return;
    }
    case "item_collection_items":
      return mColls.find((c) => c.id === a.id)?.items ?? [];
    case "outline_deck": {
      const lines = String(a.text).split("\n").map((l) => l.replace(/^\s*([-•*]|\d+[.)])\s*/, "").trim()).filter(Boolean);
      const slides: DeckSlide[] = [{ kind: "text", title: lines[0] ?? "Sermon", body: "", image: null, seconds: null }];
      for (const l of lines.slice(1)) {
        slides.push({ kind: "text", title: "", body: l, image: null, seconds: null });
        if (/John 3:16/i.test(l)) slides.push({ kind: "text", title: "John 3:16", body: "16 For God so loved the world that He gave His only begotten Son…", image: null, seconds: null });
      }
      return { id: null, title: lines[0] ?? "Sermon", looping: false, seconds: 0, slides };
    }
    case "quick_song": {
      const t = String(a.typed).trim().toUpperCase().replace(/\s+/g, " ");
      const want = /^\d+$/.test(t) ? `MHB ${t}` : t.replace(/^([A-Z]+)\s*(\d+)$/, "$1 $2");
      return SONGS.filter((s) => s.hit.numbers.includes(want)).map((s) => s.hit);
    }
    case "theme_gallery": {
      const r = await fetch("/samples/gallery.json");
      return (await r.json()) as Theme[];
    }
    case "save_theme_rules":
      mRules = a.rules as ThemeRule[];
      return;
    case "resolve_theme":
      return mResolve(a.item as ProgramItem);
    case "set_media_options": {
      const m = mMedia.find((x) => x.id === a.id);
      if (m) (m.repeat = a.repeat as MediaItem["repeat"]), (m.fit = a.fit as MediaItem["fit"]);
      return;
    }
    case "reveal_media":
      return;
    case "remote_control_key":
      return a.on ? "3f9a0c1d2e4b5a6978c0d1e2f3a4b5c6d7e8" : null;
    case "remote_control_key_get":
      return null;
    case "midi_ports":
      return ["Akai LPD8", "Loopback MIDI"];
    case "midi_use":
      mMidi.port = (a.port as string) ?? null;
      return;
    case "midi_learn":
      mMidi.learning = String(a.action);
      setTimeout(() => ((mMidi.map[mMidi.learning!] = "note 36"), (mMidi.learning = null)), 600);
      return;
    case "midi_forget":
      delete mMidi.map[String(a.action)];
      return;
    case "midi_status":
      return { ...mMidi, map: { ...mMidi.map } };
    case "speech_save_notes":
      return 42;
    case "speech_test_recording":
      return { transcript: [[2, "please turn with me to the book of Romans"], [6, "chapter eight and verse one"], [14, "there is therefore now no condemnation to those who are in Christ Jesus"]], found: [[6, "Romans 8:1", "said: “chapter eight and verse one”"]], model: "small.en", seconds: 20, took_seconds: 9 };
    case "set_window_look":
    case "show_main_window":
      return;
    case "open_ndi_runtime_page":
      window.open("https://ndi.link/NDIRedistV6", "_blank");
      return;
    case "ndi_stop":
      return;
    case "list_announcements":
      return mDecks.map((d) => ({ id: d.id!, title: d.title, slides: d.slides.length, looping: d.looping }));
    case "get_announcement":
      return structuredClone(mDecks.find((d) => d.id === a.id));
    case "announcement_templates":
      return structuredClone(TEMPLATES);
    case "save_announcement": {
      const d = a.deck as Deck;
      if (!d.title.trim()) throw new Error("Announcements need a name.");
      if (!d.slides.length) throw new Error("Add at least one slide.");
      d.slides.forEach((x, i) => {
        if (x.kind === "picture" && !x.image) throw new Error(`Slide ${i + 1} needs a picture.`);
      });
      const id = d.id ?? Math.max(0, ...mDecks.map((x) => x.id!)) + 1;
      mDecks = [...mDecks.filter((x) => x.id !== id), { ...structuredClone(d), id }];
      return id;
    }
    case "delete_announcement":
      if (mItem?.kind === "announcement" && mItem.presentation_id === a.id) throw new Error("These announcements are on the screens. Send something else live first.");
      mDecks = mDecks.filter((x) => x.id !== a.id);
      return;
    case "preview_announcement":
      return deckSlides(mDecks.find((d) => d.id === a.id)!);
    case "set_auto_paused":
      if (!mState.auto) throw new Error("Nothing is advancing by itself.");
      return bump({ auto: { ...mState.auto, paused: !!a.paused }, slide_since_ms: Date.now() });
    case "set_repeat": {
      const l = a.label as string | null;
      const on = mSlides[mState.slide_idx]?.label === l;
      return bump({ repeat: l ? { label: l, total: Math.max(1, a.total as number), done: (a.done as number | null) ?? (on ? 1 : 0) } : null });
    }
    case "set_lining_out": {
      const on = !!a.on;
      if (on && !mState.lining_out) mBeforeLining = mState.lines_per_slide;
      const lines = on ? 1 : mBeforeLining;
      const cur = mSlides[mState.slide_idx];
      if (mItem?.kind === "song") mSlides = splitSong(mItem.song_id, lines, mState.lang_mode);
      const idx = cur ? Math.max(0, mSlides.findIndex((x) => x.occurrence === cur.occurrence && x.first_key <= cur.first_key && cur.first_key <= x.last_key)) : 0;
      return bump({ lines_per_slide: lines, lining_out: on, slide_idx: idx });
    }
    case "timer_start": {
      const now = Date.now();
      const ends = a.mode === "countdown" ? (mParseLength(String(a.value)) ?? NaN) + now : mParseClock(String(a.value));
      if (ends == null || isNaN(ends)) throw new Error("Use a length like 5:00, or a time like 09:00 or 9:30 am.");
      return bump({ timer: { label: String(a.label).trim(), ends_at_ms: ends, paused_left_ms: null, count_over: true, on_screens: a.onScreens as TimerShow, on_stage: !!a.onStage } });
    }
    case "timer_control": {
      const t = mState.timer;
      if (!t) throw new Error("No timer is running.");
      const now = Date.now();
      const left = t.paused_left_ms ?? t.ends_at_ms - now;
      const next: Timer | null =
        a.action === "pause" ? { ...t, paused_left_ms: left }
        : a.action === "resume" ? (t.paused_left_ms != null ? { ...t, ends_at_ms: now + t.paused_left_ms, paused_left_ms: null } : t)
        : a.action === "add" ? (t.paused_left_ms != null ? { ...t, paused_left_ms: t.paused_left_ms + ((a.ms as number) ?? 60000) } : { ...t, ends_at_ms: t.ends_at_ms + ((a.ms as number) ?? 60000) })
        : a.action === "show" ? { ...t, on_screens: (a.onScreens as TimerShow) ?? t.on_screens, on_stage: (a.onStage as boolean) ?? t.on_stage }
        : null;
      return bump({ timer: next });
    }
    case "alert_show": {
      let text = String(a.text);
      for (const name of [...text.matchAll(/\{([^}]+)\}/g)].map((m) => m[1].trim())) {
        const v = (a.values as [string, string][]).find(([k]) => k === name)?.[1]?.trim();
        if (!v) throw new Error(`Fill in “${name}” first.`);
        text = text.split(`{${name}}`).join(v);
      }
      if (!a.onScreens && !a.onStream && !a.onStage) throw new Error("Choose at least one place to show the alert.");
      const secs = a.seconds as number | null;
      const alert: Alert = { text, style: a.style as Alert["style"], position: a.position as Alert["position"], until_ms: secs ? Date.now() + secs * 1000 : null, on_screens: !!a.onScreens, on_stream: !!a.onStream, on_stage: !!a.onStage };
      if (secs) setTimeout(() => mState.alert === alert && bump({ alert: null }), secs * 1000);
      return bump({ alert });
    }
    case "alert_clear":
      return bump({ alert: null });
    case "cue_presets":
      return { timers: structuredClone(mTimerPresets), alerts: structuredClone(mAlertPresets) };
    case "save_timer_preset": {
      const p = a.preset as TimerPreset;
      const id = p.id ?? Math.max(0, ...mTimerPresets.map((x) => x.id!)) + 1;
      mTimerPresets = [...mTimerPresets.filter((x) => x.id !== id), { ...p, id }];
      return id;
    }
    case "save_alert_preset": {
      const p = a.preset as AlertPreset;
      const id = p.id ?? Math.max(0, ...mAlertPresets.map((x) => x.id!)) + 1;
      mAlertPresets = [...mAlertPresets.filter((x) => x.id !== id), { ...p, id }];
      return id;
    }
    case "delete_cue_preset":
      if (a.kind === "timer") mTimerPresets = mTimerPresets.filter((x) => x.id !== a.id);
      else mAlertPresets = mAlertPresets.filter((x) => x.id !== a.id);
      return;
    case "get_theme": {
      const t = mThemes.find((x) => x.id === a.id);
      if (!t) throw new Error("That theme no longer exists.");
      return structuredClone(t.theme);
    }
    case "export_theme":
      return 1234;
    case "import_theme": {
      const src = mThemes.find((x) => x.kind === "song")!;
      const name = `${src.name} (imported)`;
      const id = Math.max(...mThemes.map((x) => x.id)) + 1;
      mThemes.push({ id, name, kind: "song", is_default: false, theme: { ...structuredClone(src.theme), name } });
      themesChanged();
      return { id, name };
    }
    case "set_stage_message": {
      const m = (a.message as string | null)?.trim();
      return bump({ stage_message: m ? m : null });
    }
    case "list_media": {
      const q = String(a.query).toLowerCase();
      return mMedia.filter((m) => (!a.kind || m.kind === a.kind) && m.title.toLowerCase().includes(q));
    }
    case "import_media":
      return (a.paths as string[]).map((p) => {
        const existing = mMedia.find((m) => m.path === p);
        if (existing) return { file: p, item: existing, error: null };
        const item: MediaItem = { id: Math.max(0, ...mMedia.map((m) => m.id)) + 1, kind: p.endsWith(".webm") ? "video" : "image", path: p, title: p.split("/").pop()!.replace(/\.\w+$/, ""), duration_ms: null, width: null, height: null, thumb: null, tags: null, bytes: 1, missing: false };
        mMedia.unshift(item);
        return { file: p, item, error: null };
      });
    case "set_media_meta":
      mMedia = mMedia.map((m) => (m.id === a.id ? { ...m, duration_ms: a.durationMs as number, width: a.width as number, height: a.height as number } : m));
      return;
    case "save_media_thumb": {
      const bytes = new Uint8Array(a.jpeg as number[]);
      let bin = "";
      bytes.forEach((b) => (bin += String.fromCharCode(b)));
      const url = `data:image/jpeg;base64,${btoa(bin)}`;
      mMedia = mMedia.map((m) => (m.id === a.id ? { ...m, thumb: url } : m));
      return url;
    }
    case "rename_media":
      mMedia = mMedia.map((m) => (m.id === a.id ? { ...m, title: String(a.title) } : m));
      return;
    case "media_used_by":
      return mThemes.filter((t) => JSON.stringify(t.theme).includes(`media:${mMedia.find((m) => m.id === a.id)?.path}`)).map((t) => t.name);
    case "delete_media":
      mMedia = mMedia.filter((m) => m.id !== a.id);
      return;
    case "media_control":
      return bump({ ...(a.playing != null ? { media_playing: a.playing as boolean } : {}), ...(a.restart ? { media_restart: mState.media_restart + 1, media_playing: true } : {}) });
    case "current_profile": {
      const p = mProfiles.find((x) => x.active)!;
      return { slug: p.slug, name: p.name };
    }
    case "create_profile": {
      const name = String(a.name).trim();
      if (!name) throw new Error("A profile needs a name.");
      if (mProfiles.some((x) => x.name.toLowerCase() === name.toLowerCase())) throw new Error(`There is already a profile called “${name}”.`);
      const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      const cur = mProfiles.find((x) => x.active)!;
      mProfiles.push({ slug, name, active: false, songs: a.copyCurrent ? cur.songs : 0, bibles: a.copyCurrent ? cur.bibles : 0 });
      return slug;
    }
    case "rename_profile":
      mProfiles = mProfiles.map((x) => (x.slug === a.slug ? { ...x, name: String(a.name) } : x));
      return;
    case "delete_profile":
      if (mProfiles.find((x) => x.slug === a.slug)?.active) throw new Error("Switch to another profile before deleting this one.");
      mProfiles = mProfiles.filter((x) => x.slug !== a.slug);
      return "…/profiles/.deleted/";
    case "switch_profile":
      mProfiles = mProfiles.map((x) => ({ ...x, active: x.slug === a.slug }));
      mItem = null;
      mSlides = [];
      bump({ mode: "blank", item_idx: null });
      window.dispatchEvent(new Event("wp-profile-changed"));
      return;
    case "list_backups":
      return mBackups;
    case "backup_now": {
      const b = { path: "C:\\…\\backups\\library-manual.db", file: `library-2026110${mBackups.length}-manual.db`, taken: new Date().toISOString().slice(0, 16).replace("T", " "), reason: "manual", bytes: 41_200_000 };
      mBackups.unshift(b);
      return b;
    }
    case "inspect_backup":
      return { songs: 4531, bibles: 20, schedules: 6 };
    case "restore_backup":
      return { ...mBackups[0], reason: "before-restore" };
    case "backups_folder":
      return "C:\\Users\\you\\AppData\\Roaming\\app.worshippresenter.desktop\\profiles\\default\\backups";
    case "import_file": {
      await new Promise((r) => setTimeout(r, 600));
      const k = a.kind as ImportKind;
      return k === "ew-songs"
        ? { name: "Songs", read: 4531, imported: 4531, skipped: 0, fixed_chars: 529, notes: [] }
        : k === "ewb"
          ? { name: "New King James Version", read: 31102, imported: 31102, skipped: 0, fixed_chars: 0, notes: [] }
          : { name: "Twerɛ Kronkron (2012)", read: 35477, imported: 35477, skipped: 0, fixed_chars: 0, notes: ["11752 footnotes not imported (not projected in this version)"] };
    }
  }
  throw new Error(`Unknown command ${cmd}`);
}
