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
}

export interface LiveState {
  revision: number;
  item_idx: number | null;
  slide_idx: number;
  mode: Mode;
  lang_mode: LangMode;
  lines_per_slide: number;
}

export type ProgramItem =
  | { kind: "song"; song_id: number; title: string }
  | { kind: "scripture"; reference: string; versions: string[] };

export interface Broadcast {
  state: LiveState;
  item: ProgramItem | null;
  slides: Slide[];
  caption: string | null;
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
};

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
    sections: [{ label: "V1", en: ["Come, let us join our cheerful songs", "With angels round the throne;", "Ten thousand thousand are their tongues,", "But all their joys are one."] }],
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

let mState: LiveState = { revision: 0, item_idx: null, slide_idx: 0, mode: "blank", lang_mode: "both", lines_per_slide: 2 };
let mItem: ProgramItem | null = null;
let mSlides: Slide[] = [];
let mCaption: string | null = null;
let mPrev: { item: ProgramItem | null; slides: Slide[]; idx: number } | null = null;
let chan: BroadcastChannel | null = null;

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
        primary: mode === "second" && sec.fat ? [] : sec.en.slice(i, i + lines),
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

function emit() {
  chan ??= new BroadcastChannel("wp-live");
  const payload: Broadcast = { state: mState, item: mItem, slides: mSlides, caption: mCaption };
  chan.postMessage({ type: "live", payload });
  window.dispatchEvent(new CustomEvent("wp-local-live", { detail: payload }));
}

if (!inTauri && typeof window !== "undefined") {
  chan = new BroadcastChannel("wp-live");
  // the tab that holds a live item answers resync requests from output tabs
  chan.addEventListener("message", (e) => e.data.type === "resync" && mItem && emit());
  setInterval(() => chan?.postMessage({ type: "heartbeat", payload: mState.revision }), 1000);
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
      mPrev = { item: mItem, slides: mSlides, idx: mState.slide_idx };
      const item = a.item as ProgramItem;
      if (item.kind === "song") {
        mSlides = splitSong(item.song_id, mState.lines_per_slide, mState.lang_mode);
        mCaption = item.title;
      } else {
        [mSlides, mCaption] = scripture(item.reference, item.versions);
      }
      mItem = item;
      return bump({ item_idx: 0, slide_idx: Math.min(a.slideIdx as number, mSlides.length - 1), mode: "live" });
    }
    case "show_slide": {
      if (!mSlides.length) throw new Error("Nothing is live. Send an item to Live first.");
      const idx = a.slideIdx != null ? (a.slideIdx as number) : mState.slide_idx + ((a.delta as number) ?? 0);
      const mode = mState.mode === "blank" || mState.mode === "logo" ? "live" : mState.mode;
      return bump({ slide_idx: Math.max(0, Math.min(mSlides.length - 1, idx)), mode });
    }
    case "set_mode": {
      const m = a.mode as Mode;
      return bump({ mode: mState.mode === m && m !== "live" ? "live" : m });
    }
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
      return;
  }
  throw new Error(`Unknown command ${cmd}`);
}
