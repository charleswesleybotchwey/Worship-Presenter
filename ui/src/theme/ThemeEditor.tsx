import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, pickForeignTheme, pickThemeOpen, pickThemeSave, type ForeignDesign, type MediaItem } from "../api";
import { MediaGrid } from "../MediaLibrary";
import { FontPicker } from "./FontPicker";
import { ForeignThemeImport } from "./ForeignThemeImport";
import { ThemeView, type Phase } from "./ThemeView";
import {
  FIELD_NAMES,
  KIND_NAMES,
  blankTheme,
  newId,
  newLayer,
  type Anim,
  type AnimKind,
  type Field,
  type Layer,
  type SlideData,
  type TextLayer,
  type TextStyle,
  type Theme,
  type ThemeKind,
  type ThemeRow,
} from "./types";

// Theme editor: ProPresenter-style looks built from vMix-style layers.

const SAMPLES: Record<string, { name: string; data: SlideData }> = {
  bilingual: {
    name: "Hymn, English and Fante",
    data: { primary: ["O for a thousand tongues to sing", "My great Redeemer’s praise,"], second: ["Meenya ngyiramá apem matow ndwom", "Meyi mo Pomfò ayɛw,"], title: "MHB 1", copyright: "Public Domain", section: "Verse 1" },
  },
  single: { name: "Song, one language", data: { primary: ["Great is Thy faithfulness", "O God my Father"], second: [], title: "Great is Thy faithfulness", section: "Chorus" } },
  scripture: {
    name: "Scripture, two versions",
    data: {
      primary: ["For God so loved the world that He gave His only begotten Son, that whoever believes in Him should not perish but have everlasting life."],
      second: ["Na sɛdeɛ Onyankopɔn dɔɔ wiase nie, sɛ ɔde ne Ba a ɔwoo no korɔ no maeɛ, na obiara a ɔgye no die no anyera, na wanya daa nkwa."],
      reference: "Yohane 3:16",
    },
  },
  long: {
    name: "Very long verse",
    data: {
      primary: [
        "Now it came to pass, in the days when the judges ruled, that there was a famine in the land. And a certain man of Bethlehem, Judah, went to dwell in the country of Moab, he and his wife and his two sons. The name of the man was Elimelech, the name of his wife was Naomi.",
      ],
      second: [],
      reference: "Ruth 1:1-2",
    },
  },
};
const sampleFor = (k: ThemeKind) => (k === "scripture" ? "scripture" : "bilingual");

const ANIMS: { value: AnimKind; label: string }[] = [
  { value: "none", label: "None" },
  { value: "fade", label: "Fade" },
  { value: "slide-up", label: "Slide up" },
  { value: "slide-down", label: "Slide down" },
  { value: "slide-left", label: "Slide from right" },
  { value: "slide-right", label: "Slide from left" },
  { value: "wipe", label: "Wipe" },
  { value: "scale", label: "Grow" },
];

const r1 = (n: number) => Math.round(n * 10) / 10;

const hasSectionLabel = (t: Theme) => t.layers.some((l) => l.kind === "text" && (l as TextLayer).field === "section");

/** Adds a small "Verse 1 / Chorus" label above the words (styled like the lyrics), or removes it. */
function withSectionLabel(t: Theme, on: boolean): Theme {
  if (!on) return { ...t, layers: t.layers.filter((l) => !(l.kind === "text" && (l as TextLayer).field === "section")) };
  if (hasSectionLabel(t)) return t;
  const lyrics = t.layers.find((l) => l.kind === "text" && (l as TextLayer).field === "lyrics") as TextLayer | undefined;
  const base = lyrics ?? (t.layers.find((l) => l.kind === "text") as TextLayer | undefined);
  if (!base) return t;
  const top = Math.max(1, base.y - 9);
  const label: TextLayer = {
    ...structuredClone(base),
    id: `section-${Date.now().toString(36)}`,
    name: "Section label",
    field: "section",
    text: "",
    solo: null,
    x: base.x,
    y: top,
    w: base.w,
    h: 7,
    style: { ...structuredClone(base.style), size: Math.max(2, Math.round(base.style.size * 0.55 * 10) / 10), minSize: 1.5, autoFit: false, weight: 600, italic: false },
  };
  return { ...t, layers: [...t.layers, label] };
}

/** NDI keys the green behind the lower third to transparency; strong greens in the
 *  theme itself would turn partly see-through. */
function greenWarning(t: Theme): string | null {
  const greenness = (hex: string) => {
    const m = hex.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i);
    if (!m) return 0;
    const [r, g, b] = [1, 2, 3].map((i) => parseInt(m[i], 16));
    return g - Math.max(r, b);
  };
  const colours: [string, string][] = [];
  for (const l of t.layers) {
    if (l.kind === "shape") colours.push([l.name, l.fill]);
    if (l.kind === "text" || l.kind === "ticker") colours.push([l.name, l.style.color]);
  }
  const bad = colours.find(([, c]) => greenness(c) > 8);
  return bad ? `“${bad[0]}” uses a strong green; over NDI it would turn partly see-through.` : null;
}

// ---------- small inputs ----------

function Num({ label, value, onChange, step = 0.5, min, max, suffix }: { label: string; value: number; onChange: (v: number) => void; step?: number; min?: number; max?: number; suffix?: string }) {
  return (
    <label className="prop">
      <span>{label}</span>
      <span className="prop__num">
        <input type="number" value={r1(value)} step={step} min={min} max={max} onChange={(e) => e.target.value !== "" && onChange(Number(e.target.value))} />
        {suffix && <em>{suffix}</em>}
      </span>
    </label>
  );
}

function hexParts(c: string): [string, number] {
  const m = c.match(/^#([0-9a-f]{6})([0-9a-f]{2})?$/i);
  if (!m) return ["#000000", 100];
  return [`#${m[1]}`, m[2] ? Math.round((parseInt(m[2], 16) / 255) * 100) : 100];
}

function Color({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  const [hex, alpha] = hexParts(value);
  const join = (h: string, a: number) => (a >= 100 ? h : `${h}${Math.round((a / 100) * 255).toString(16).padStart(2, "0")}`);
  return (
    <label className="prop">
      <span>{label}</span>
      <span className="prop__color">
        <input type="color" value={hex} onChange={(e) => onChange(join(e.target.value, alpha))} />
        <input type="range" min={0} max={100} value={alpha} onChange={(e) => onChange(join(hex, Number(e.target.value)))} aria-label={`${label} opacity`} />
        <em>{alpha}%</em>
      </span>
    </label>
  );
}

function Select<T extends string | number>({ label, value, options, onChange }: { label: string; value: T; options: { value: T; label: string }[]; onChange: (v: T) => void }) {
  return (
    <label className="prop">
      <span>{label}</span>
      <select value={value} onChange={(e) => onChange((typeof value === "number" ? Number(e.target.value) : e.target.value) as T)}>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function Check({ label, value, onChange }: { label: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="prop prop--check">
      <input type="checkbox" checked={value} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="prop-group">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

/** Reads an image file as a data URL, scaled down so themes stay small. */
async function readImage(file: File, maxW = 1920): Promise<string> {
  const url = URL.createObjectURL(file);
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    const scale = Math.min(1, maxW / img.naturalWidth);
    const c = document.createElement("canvas");
    c.width = Math.round(img.naturalWidth * scale);
    c.height = Math.round(img.naturalHeight * scale);
    c.getContext("2d")!.drawImage(img, 0, 0, c.width, c.height);
    const png = file.type === "image/png" || file.type === "image/webp" || file.type === "image/svg+xml";
    return c.toDataURL(png ? "image/png" : "image/jpeg", 0.88);
  } finally {
    URL.revokeObjectURL(url);
  }
}

function ImagePick({ label, onPick }: { label: string; onPick: (dataUrl: string) => void }) {
  const ref = useRef<HTMLInputElement>(null);
  return (
    <>
      <button className="btn" onClick={() => ref.current?.click()}>
        {label}
      </button>
      <input
        ref={ref}
        type="file"
        accept="image/png,image/jpeg,image/webp"
        hidden
        onChange={async (e) => {
          const f = e.target.files?.[0];
          if (f) onPick(await readImage(f));
          e.target.value = "";
        }}
      />
    </>
  );
}

/** Choose a picture or video from the profile's media library. */
function LibraryPick({ label, kind, onPick }: { label: string; kind: "image" | "video"; onPick: (m: MediaItem) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button className="btn" onClick={() => setOpen(true)}>
        {label}
      </button>
      {open && (
        <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && setOpen(false)}>
          <div className="dialog dialog--picker" role="dialog" aria-label={label}>
            <div className="dialog__head">
              <span className="pane__title">{kind === "video" ? "Choose a video" : "Choose a picture"}</span>
              <button className="btn-quiet" onClick={() => setOpen(false)}>
                Cancel
              </button>
            </div>
            <MediaGrid kind={kind} onPick={(m) => (onPick(m), setOpen(false))} />
          </div>
        </div>
      )}
    </>
  );
}

const libRef = (m: MediaItem) => `media:${m.path}`;
const thumbRef = (m: MediaItem) => (m.thumb ? (m.thumb.startsWith("data:") ? m.thumb : `media:${m.thumb}`) : null);

function AnimProps({ title, value, onChange }: { title: string; value: Anim; onChange: (a: Anim) => void }) {
  return (
    <Group title={title}>
      <Select label="Motion" value={value.kind} options={ANIMS} onChange={(kind) => onChange({ ...value, kind })} />
      {value.kind !== "none" && (
        <div className="prop-row">
          <Num label="Length" value={value.ms} step={50} min={0} suffix="ms" onChange={(ms) => onChange({ ...value, ms })} />
          <Num label="Delay" value={value.delay} step={50} min={0} suffix="ms" onChange={(delay) => onChange({ ...value, delay })} />
        </div>
      )}
    </Group>
  );
}

function TextStyleProps({ s, set }: { s: TextStyle; set: (p: Partial<TextStyle>) => void }) {
  return (
    <Group title="Text">
      <FontPicker value={s.font} onChange={(font) => set({ font })} />
      <div className="prop-row">
        <Num label="Size" value={s.size} step={0.25} min={0.5} suffix="%" onChange={(size) => set({ size })} />
        <Select
          label="Weight"
          value={s.weight}
          options={[300, 400, 500, 600, 700, 800].map((w) => ({ value: w, label: { 300: "Light", 400: "Regular", 500: "Medium", 600: "Semibold", 700: "Bold", 800: "Heavy" }[w]! }))}
          onChange={(weight) => set({ weight })}
        />
      </div>
      <Color label="Colour" value={s.color} onChange={(color) => set({ color })} />
      <div className="prop-row">
        <Select label="Across" value={s.align} options={[{ value: "left", label: "Left" }, { value: "center", label: "Centre" }, { value: "right", label: "Right" }]} onChange={(align) => set({ align })} />
        <Select label="Down" value={s.valign} options={[{ value: "top", label: "Top" }, { value: "middle", label: "Middle" }, { value: "bottom", label: "Bottom" }]} onChange={(valign) => set({ valign })} />
      </div>
      <div className="prop-row">
        <Num label="Line height" value={s.lineHeight} step={0.05} min={0.8} onChange={(lineHeight) => set({ lineHeight })} />
        <Num label="Letter spacing" value={s.letterSpacing} step={0.01} onChange={(letterSpacing) => set({ letterSpacing })} suffix="em" />
      </div>
      <div className="prop-row">
        <Check label="Italic" value={s.italic} onChange={(italic) => set({ italic })} />
        <Check label="Capitals" value={s.uppercase} onChange={(uppercase) => set({ uppercase })} />
      </div>
      <Check label="Shrink to fit the box" value={s.autoFit} onChange={(autoFit) => set({ autoFit })} />
      {s.autoFit && <Num label="Smallest size" value={s.minSize} step={0.25} min={0.5} suffix="%" onChange={(minSize) => set({ minSize })} />}
      <Check label="Shadow" value={s.shadow.on} onChange={(on) => set({ shadow: { ...s.shadow, on } })} />
      {s.shadow.on && (
        <>
          <Color label="Shadow colour" value={s.shadow.color} onChange={(color) => set({ shadow: { ...s.shadow, color } })} />
          <div className="prop-row">
            <Num label="Blur" value={s.shadow.blur} step={0.1} min={0} onChange={(blur) => set({ shadow: { ...s.shadow, blur } })} />
            <Num label="Drop" value={s.shadow.y} step={0.05} onChange={(y) => set({ shadow: { ...s.shadow, y } })} />
          </div>
        </>
      )}
      <Check label="Outline" value={s.outline.on} onChange={(on) => set({ outline: { ...s.outline, on } })} />
      {s.outline.on && (
        <div className="prop-row">
          <Color label="Outline colour" value={s.outline.color} onChange={(color) => set({ outline: { ...s.outline, color } })} />
          <Num label="Width" value={s.outline.width} step={0.05} min={0} onChange={(width) => set({ outline: { ...s.outline, width } })} />
        </div>
      )}
    </Group>
  );
}

// ---------- editor ----------

type Drag = { id: string; handle: string | null; px: number; py: number; start: { x: number; y: number; w: number; h: number }; box: DOMRect };

export function ThemeEditor({ onClose }: { onClose: () => void }) {
  const [rows, setRows] = useState<ThemeRow[]>([]);
  const [currentId, setCurrentId] = useState<number | null>(null);
  const [theme, setTheme] = useState<Theme | null>(null);
  const [saved, setSaved] = useState<string>("");
  const [selected, setSelected] = useState<string | null>(null);
  const [sample, setSample] = useState("bilingual");
  const [phase, setPhase] = useState<Phase>("static");
  const [showKey, setShowKey] = useState(0);
  const [guides, setGuides] = useState<{ x: number[]; y: number[] }>({ x: [], y: [] });
  const [message, setMessage] = useState<{ text: string; bad?: boolean } | null>(null);
  const [foreign, setForeign] = useState<{ path: string; designs: ForeignDesign[] } | null>(null);
  const history = useRef<Theme[]>([]);
  const drag = useRef<Drag | null>(null);
  const canvas = useRef<HTMLDivElement>(null);

  const dirty = theme != null && JSON.stringify(theme) !== saved;

  const refresh = useCallback(async (selectId?: number) => {
    const r = await api.listThemes();
    setRows(r);
    const pick = r.find((x) => x.id === selectId) ?? r.find((x) => x.kind === "song" && x.is_default) ?? r[0];
    if (pick && selectId !== undefined) open(pick);
    return r;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const open = (row: ThemeRow) => {
    setCurrentId(row.id);
    setTheme(structuredClone(row.theme));
    setSaved(JSON.stringify(row.theme));
    setSelected(null);
    setSample(sampleFor(row.kind));
    setMessage(null);
    setPhase("static");
    history.current = [];
  };

  useEffect(() => {
    api.listThemes().then((r) => {
      setRows(r);
      const first = r.find((x) => x.kind === "song" && x.is_default) ?? r[0];
      if (first) open(first);
    });
  }, []);

  const change = useCallback((next: Theme) => {
    setTheme((prev) => {
      if (prev) history.current = [...history.current.slice(-99), prev];
      return next;
    });
  }, []);
  const layer = theme?.layers.find((l) => l.id === selected) ?? null;
  const setLayer = (p: Partial<Layer>) => theme && layer && change({ ...theme, layers: theme.layers.map((l) => (l.id === layer.id ? ({ ...l, ...p } as Layer) : l)) });
  const setStyle = (p: Partial<TextStyle>) => layer && "style" in layer && setLayer({ style: { ...layer.style, ...p } } as Partial<Layer>);

  const confirmLeave = () => !dirty || confirm("Leave without saving your changes to this theme?");

  const save = async () => {
    if (!theme) return;
    try {
      const id = await api.saveTheme(currentId, theme);
      setCurrentId(id);
      setSaved(JSON.stringify(theme));
      setMessage({ text: "Saved. Outputs using this theme have updated." });
      setRows(await api.listThemes());
    } catch (e) {
      setMessage({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
  };

  const act = async (f: () => Promise<unknown>, ok?: string) => {
    try {
      await f();
      if (ok) setMessage({ text: ok });
      return true;
    } catch (e) {
      setMessage({ text: String(e instanceof Error ? e.message : e), bad: true });
      return false;
    }
  };

  // ---------- canvas dragging with snapping ----------

  const onPointerDown = (e: React.PointerEvent, l: Layer, handle: string | null) => {
    e.preventDefault();
    setSelected(l.id);
    if (l.locked || !canvas.current) return;
    const box = canvas.current.querySelector(".tv")!.getBoundingClientRect();
    drag.current = { id: l.id, handle, px: e.clientX, py: e.clientY, start: { x: l.x, y: l.y, w: l.w, h: l.h }, box };
    history.current = theme ? [...history.current.slice(-99), theme] : history.current;
  };

  useEffect(() => {
    const move = (e: PointerEvent) => {
      const d = drag.current;
      if (!d || !theme) return;
      const dx = ((e.clientX - d.px) / d.box.width) * 100;
      const dy = ((e.clientY - d.py) / d.box.height) * 100;
      let { x, y, w, h } = d.start;
      const hd = d.handle;
      if (!hd) {
        x += dx;
        y += dy;
      } else {
        if (hd.includes("e")) w = Math.max(1, w + dx);
        if (hd.includes("s")) h = Math.max(1, h + dy);
        if (hd.includes("w")) (x += dx), (w = Math.max(1, w - dx));
        if (hd.includes("n")) (y += dy), (h = Math.max(1, h - dy));
      }
      // snap edges and centres to the canvas and to other layers (hold Alt to place freely)
      const gx: number[] = [];
      const gy: number[] = [];
      if (!e.altKey) {
        const tx = [0, 50, 100, ...theme.layers.filter((l) => l.id !== d.id && !l.hidden).flatMap((l) => [l.x, l.x + l.w / 2, l.x + l.w])];
        const ty = [0, 50, 100, ...theme.layers.filter((l) => l.id !== d.id && !l.hidden).flatMap((l) => [l.y, l.y + l.h / 2, l.y + l.h])];
        const snap = (vals: [number, (t: number) => void][], targets: number[], out: number[]) => {
          let best: [number, () => void, number] | null = null;
          for (const [v, apply] of vals)
            for (const t of targets) {
              const dist = Math.abs(v - t);
              if (dist < 0.8 && (!best || dist < best[0])) best = [dist, () => apply(t), t];
            }
          if (best) best[1](), out.push(best[2]);
        };
        if (!hd) {
          snap([[x, (t) => (x = t)], [x + w / 2, (t) => (x = t - w / 2)], [x + w, (t) => (x = t - w)]], tx, gx);
          snap([[y, (t) => (y = t)], [y + h / 2, (t) => (y = t - h / 2)], [y + h, (t) => (y = t - h)]], ty, gy);
        } else {
          if (hd.includes("e")) snap([[x + w, (t) => (w = t - x)]], tx, gx);
          if (hd.includes("w")) snap([[x, (t) => ((w = w + x - t), (x = t))]], tx, gx);
          if (hd.includes("s")) snap([[y + h, (t) => (h = t - y)]], ty, gy);
          if (hd.includes("n")) snap([[y, (t) => ((h = h + y - t), (y = t))]], ty, gy);
        }
      }
      setGuides({ x: gx, y: gy });
      setTheme({ ...theme, layers: theme.layers.map((l) => (l.id === d.id ? { ...l, x: r1(x), y: r1(y), w: r1(w), h: r1(h) } : l)) });
    };
    const up = () => {
      drag.current = null;
      setGuides({ x: [], y: [] });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => (window.removeEventListener("pointermove", move), window.removeEventListener("pointerup", up));
  }, [theme]);

  // keyboard: arrows nudge, Delete removes, Ctrl+Z undo, Ctrl+S save
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement).closest("input, textarea, select");
      if (e.ctrlKey && e.key.toLowerCase() === "s") return e.preventDefault(), void save();
      if (typing) return;
      if (e.ctrlKey && e.key.toLowerCase() === "z") {
        e.preventDefault();
        const prev = history.current.pop();
        if (prev) setTheme(prev);
        return;
      }
      if (!layer || !theme || layer.locked) return;
      const step = e.shiftKey ? 5 : 0.5;
      const nudge: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
      if (nudge[e.key]) {
        e.preventDefault();
        setLayer({ x: r1(layer.x + nudge[e.key][0]), y: r1(layer.y + nudge[e.key][1]) });
      } else if (e.key === "Delete") {
        change({ ...theme, layers: theme.layers.filter((l) => l.id !== layer.id) });
        setSelected(null);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const play = (p: Phase) => {
    setPhase(p);
    if (p === "in") setShowKey((k) => k + 1);
  };

  const data = useMemo(() => SAMPLES[sample].data, [sample]);
  const kinds: ThemeKind[] = ["song", "scripture", "lower_third"];

  if (!theme) {
    return (
      <div className="te">
        <p className="empty">Loading themes…</p>
      </div>
    );
  }

  const moveLayer = (id: string, d: number) => {
    const i = theme.layers.findIndex((l) => l.id === id);
    const j = i + d;
    if (j < 0 || j >= theme.layers.length) return;
    const ls = [...theme.layers];
    [ls[i], ls[j]] = [ls[j], ls[i]];
    change({ ...theme, layers: ls });
  };

  return (
    <div className="te" role="dialog" aria-label="Theme editor">
      <header className="te__head">
        <span className="pane__title">Themes</span>
        <span className="te__name">{theme.name}</span>
        {dirty && <span className="te__dirty">Unsaved changes</span>}
        {theme.kind === "lower_third" && greenWarning(theme) && <span className="te__warn" title="NDI keys pure green to transparency">{greenWarning(theme)}</span>}
        <span className="topbar__spacer" />
        {message && <span className={message.bad ? "status__error" : "te__ok"}>{message.text}</span>}
        <button className="btn-quiet" onClick={() => confirmLeave() && onClose()}>
          Close
        </button>
        <button className="btn" onClick={save} disabled={!dirty}>
          Save <kbd>Ctrl+S</kbd>
        </button>
      </header>

      <div className="te__body">
        {/* ---------- left: themes and layers ---------- */}
        <aside className="te__side">
          {kinds.map((k) => (
            <section key={k} className="te__group">
              <h3>{KIND_NAMES[k]}</h3>
              <ul>
                {rows
                  .filter((r) => r.kind === k)
                  .map((r) => (
                    <li key={r.id}>
                      <button className={`te__theme ${r.id === currentId ? "is-current" : ""}`} onClick={() => r.id !== currentId && confirmLeave() && open(r)}>
                        <span>{r.name}</span>
                        {r.is_default && <span className="te__badge">In use</span>}
                      </button>
                    </li>
                  ))}
              </ul>
              <button
                className="btn-quiet te__add"
                onClick={async () => {
                  if (!confirmLeave()) return;
                  const t = blankTheme(k);
                  const id = await api.saveTheme(null, t);
                  await refresh(id);
                }}
              >
                + New {KIND_NAMES[k].toLowerCase()} theme
              </button>
            </section>
          ))}
          {currentId != null && (
            <div className="te__actions">
              <button
                className="btn-quiet"
                disabled={dirty || rows.find((r) => r.id === currentId)?.is_default}
                title={dirty ? "Save first" : undefined}
                onClick={() => act(() => api.setDefaultTheme(theme.kind, currentId), `“${theme.name}” is now used for ${KIND_NAMES[theme.kind].toLowerCase()}.`).then(() => api.listThemes().then(setRows))}
              >
                Use this theme
              </button>
              <button className="btn-quiet" onClick={async () => confirmLeave() && refresh(await api.duplicateTheme(currentId))}>
                Duplicate
              </button>
              <button
                className="btn-quiet"
                disabled={dirty}
                title={dirty ? "Save first" : "Save as a .wptheme file, with its pictures and videos, to use on another computer"}
                onClick={async () => {
                  const path = await pickThemeSave(theme.name);
                  if (!path) return;
                  await act(async () => {
                    const bytes = await api.exportTheme(currentId, path);
                    setMessage({ text: `Exported (${bytes > 1e6 ? (bytes / 1e6).toFixed(1) + " MB" : Math.max(1, Math.round(bytes / 1e3)) + " KB"}). Copy the file to the other computer and use Import there.` });
                  });
                }}
              >
                Export…
              </button>
              <button
                className="btn-quiet btn-danger"
                onClick={async () => {
                  if (!confirm(`Delete the theme “${theme.name}”?`)) return;
                  if (await act(() => api.deleteTheme(currentId))) {
                    const r = await api.listThemes();
                    setRows(r);
                    const next = r.find((x) => x.kind === theme.kind) ?? r[0];
                    if (next) open(next);
                  }
                }}
              >
                Delete
              </button>
            </div>
          )}

          <div className="te__actions">
            <button
              className="btn-quiet"
              onClick={async () => {
                if (!confirmLeave()) return;
                const path = await pickThemeOpen();
                if (!path) return;
                try {
                  const r = await api.importTheme(path);
                  await refresh(r.id);
                  setMessage({ text: `Imported “${r.name}”. Its pictures and videos are in the media library. Choose “Use this theme” to put it in use.` });
                } catch (e) {
                  setMessage({ text: String(e instanceof Error ? e.message : e), bad: true });
                }
              }}
            >
              Import a theme…
            </button>
            <button
              className="btn-quiet"
              onClick={async () => {
                if (!confirmLeave()) return;
                const path = await pickForeignTheme();
                if (!path) return;
                try {
                  setForeign({ path, designs: await api.foreignThemeReview(path) });
                } catch (e) {
                  setMessage({ text: String(e instanceof Error ? e.message : e), bad: true });
                }
              }}
            >
              From EasyWorship or ProPresenter…
            </button>
          </div>
          {foreign && (
            <ForeignThemeImport
              path={foreign.path}
              designs={foreign.designs}
              onCancel={() => setForeign(null)}
              onDone={async (text, first) => {
                setForeign(null);
                await refresh(first);
                setMessage({ text });
              }}
            />
          )}

          <section className="te__group te__layers">
            <h3>Layers, front first</h3>
            <ul>
              {[...theme.layers].reverse().map((l) => (
                <li key={l.id} className={l.id === selected ? "is-current" : ""}>
                  <button className="te__layer" onClick={() => setSelected(l.id)}>
                    <span className="te__layer-kind">{l.kind === "text" ? "T" : l.kind === "shape" ? "▭" : l.kind === "image" ? "◩" : "»"}</span>
                    <span className="te__layer-name">{l.name || l.kind}</span>
                  </button>
                  <button className="icon-btn" title={l.hidden ? "Show" : "Hide"} aria-pressed={!l.hidden} onClick={() => change({ ...theme, layers: theme.layers.map((x) => (x.id === l.id ? { ...x, hidden: !x.hidden } : x)) })}>
                    {l.hidden ? "◌" : "●"}
                  </button>
                  <button className="icon-btn" title="Bring forward" onClick={() => moveLayer(l.id, 1)}>
                    ↑
                  </button>
                  <button className="icon-btn" title="Send back" onClick={() => moveLayer(l.id, -1)}>
                    ↓
                  </button>
                </li>
              ))}
            </ul>
            <div className="te__addrow">
              {(["text", "shape", "image", "ticker"] as const).map((k) => (
                <button
                  key={k}
                  className="btn-quiet"
                  onClick={() => {
                    const l = newLayer(k);
                    change({ ...theme, layers: [...theme.layers, l] });
                    setSelected(l.id);
                  }}
                >
                  + {k === "text" ? "Text" : k === "shape" ? "Shape" : k === "image" ? "Image" : "Ticker"}
                </button>
              ))}
            </div>
          </section>
        </aside>

        {/* ---------- centre: canvas ---------- */}
        <main className="te__stage">
          <div className="te__stagebar">
            <Select label="Sample" value={sample} options={Object.entries(SAMPLES).map(([value, s]) => ({ value, label: s.name }))} onChange={setSample} />
            <span className="topbar__spacer" />
            <button className="btn-quiet" onClick={() => play("in")}>
              ▶ Play in
            </button>
            <button className="btn-quiet" onClick={() => play("out")}>
              Play out
            </button>
            <button className="btn-quiet" onClick={() => setPhase("static")} disabled={phase === "static"}>
              Reset
            </button>
          </div>
          <div className={`te__canvas ${theme.background.kind === "transparent" ? "te__canvas--checker" : ""}`} ref={canvas} onPointerDown={(e) => e.target === e.currentTarget && setSelected(null)}>
            <ThemeView theme={theme} data={data} phase={phase} showKey={showKey} editor={phase === "static" ? { selected, onPointerDown, guides } : undefined} />
          </div>
          <p className="hint">Drag to move, pull the handles to resize. Edges snap to the centre, the sides and other layers; hold Alt to place freely. Arrow keys nudge (Shift for bigger steps).</p>
        </main>

        {/* ---------- right: properties ---------- */}
        <aside className="te__props">
          {!layer ? (
            <>
              <Group title="Theme">
                <label className="prop">
                  <span>Name</span>
                  <input value={theme.name} onChange={(e) => change({ ...theme, name: e.target.value })} />
                </label>
                <Select label="Used for" value={theme.kind} options={kinds.map((k) => ({ value: k, label: KIND_NAMES[k] }))} onChange={(kind) => change({ ...theme, kind })} />
                {(theme.kind === "song" || theme.kind === "lower_third") && (
                  <label className="prop prop--check" title="Shows Verse 1, Chorus, Bridge… on the slide. Drag the label to move it; click it to change its font and size.">
                    <input type="checkbox" checked={hasSectionLabel(theme)} onChange={(e) => change(withSectionLabel(theme, e.target.checked))} />
                    <span>Show verse and chorus labels</span>
                  </label>
                )}
              </Group>
              <Group title="Background">
                <Select
                  label="Type"
                  value={theme.background.kind}
                  options={[
                    { value: "color", label: "Colour" },
                    { value: "image", label: "Picture" },
                    { value: "video", label: "Moving background (video)" },
                    { value: "transparent", label: "See-through (for keying)" },
                  ]}
                  onChange={(kind) => change({ ...theme, background: { ...theme.background, kind } })}
                />
                {theme.background.kind === "color" && <Color label="Colour" value={theme.background.color} onChange={(color) => change({ ...theme, background: { ...theme.background, color } })} />}
                {theme.background.kind === "video" && (
                  <>
                    <div className="prop-buttons">
                      <LibraryPick
                        label={theme.background.video ? "Change video…" : "Choose video…"}
                        kind="video"
                        onPick={(m) => change({ ...theme, background: { ...theme.background, video: libRef(m), poster: thumbRef(m) } })}
                      />
                    </div>
                    <p className="hint hint--tight">Plays silently on a loop behind the words and keeps playing through slide changes.</p>
                  </>
                )}
                {(theme.background.kind === "image" || theme.background.kind === "video") && (
                  <>
                    {theme.background.kind === "image" && (
                      <div className="prop-buttons">
                        <LibraryPick label="From media library…" kind="image" onPick={(m) => change({ ...theme, background: { ...theme.background, image: libRef(m) } })} />
                        <ImagePick label="From a file…" onPick={(image) => change({ ...theme, background: { ...theme.background, image } })} />
                      </div>
                    )}
                    <Select label="Fit" value={theme.background.fit} options={[{ value: "cover", label: "Fill the screen" }, { value: "contain", label: "Show all of it" }]} onChange={(fit) => change({ ...theme, background: { ...theme.background, fit } })} />
                    <label className="prop">
                      <span>Darken</span>
                      <input type="range" min={0} max={90} value={Math.round(theme.background.dim * 100)} onChange={(e) => change({ ...theme, background: { ...theme.background, dim: Number(e.target.value) / 100 } })} />
                    </label>
                  </>
                )}
              </Group>
              <Group title="Between slides">
                <Select label="Text change" value={theme.transition.kind} options={[{ value: "fade", label: "Fade" }, { value: "cut", label: "Cut" }]} onChange={(kind) => change({ ...theme, transition: { ...theme.transition, kind } })} />
                {theme.transition.kind === "fade" && <Num label="Length" value={theme.transition.ms} step={50} min={50} suffix="ms" onChange={(ms) => change({ ...theme, transition: { ...theme.transition, ms } })} />}
              </Group>
              <p className="hint">Select a layer on the canvas or in the list to change it.</p>
            </>
          ) : (
            <>
              <Group title="Layer">
                <label className="prop">
                  <span>Name</span>
                  <input value={layer.name} onChange={(e) => setLayer({ name: e.target.value })} />
                </label>
                <div className="prop-row">
                  <Num label="Left" value={layer.x} suffix="%" onChange={(x) => setLayer({ x })} />
                  <Num label="Top" value={layer.y} suffix="%" onChange={(y) => setLayer({ y })} />
                </div>
                <div className="prop-row">
                  <Num label="Width" value={layer.w} min={1} suffix="%" onChange={(w) => setLayer({ w })} />
                  <Num label="Height" value={layer.h} min={1} suffix="%" onChange={(h) => setLayer({ h })} />
                </div>
                <label className="prop">
                  <span>Opacity</span>
                  <input type="range" min={0} max={100} value={Math.round(layer.opacity * 100)} onChange={(e) => setLayer({ opacity: Number(e.target.value) / 100 })} />
                </label>
                <div className="prop-row">
                  <Check label="Lock position" value={layer.locked} onChange={(locked) => setLayer({ locked })} />
                  <button
                    className="btn-quiet btn-danger"
                    onClick={() => {
                      change({ ...theme, layers: theme.layers.filter((l) => l.id !== layer.id) });
                      setSelected(null);
                    }}
                  >
                    Delete layer
                  </button>
                </div>
                <button
                  className="btn-quiet"
                  onClick={() => {
                    const copy = { ...structuredClone(layer), id: newId(layer.kind), name: `${layer.name} copy`, y: Math.min(95, layer.y + 3) };
                    change({ ...theme, layers: [...theme.layers, copy] });
                    setSelected(copy.id);
                  }}
                >
                  Duplicate layer
                </button>
              </Group>

              {layer.kind === "text" && (
                <>
                  <Group title="Shows">
                    <Select label="Content" value={layer.field} options={(Object.keys(FIELD_NAMES) as Field[]).map((f) => ({ value: f, label: FIELD_NAMES[f] }))} onChange={(field) => setLayer({ field } as Partial<TextLayer>)} />
                    {layer.field === "free" && (
                      <label className="prop prop--block">
                        <span>Text</span>
                        <textarea value={layer.text} rows={3} onChange={(e) => setLayer({ text: e.target.value } as Partial<TextLayer>)} />
                      </label>
                    )}
                    {layer.field !== "second" && (
                      <Check
                        label="Take more room when there is no second language"
                        value={!!layer.solo}
                        onChange={(on) => setLayer({ solo: on ? { x: layer.x, y: layer.y, w: layer.w, h: Math.min(100 - layer.y, layer.h + 25) } : null } as Partial<TextLayer>)}
                      />
                    )}
                    {layer.solo && (
                      <div className="prop-row">
                        <Num label="Height then" value={layer.solo.h} min={1} suffix="%" onChange={(h) => setLayer({ solo: { ...layer.solo!, h } } as Partial<TextLayer>)} />
                        <Num label="Top then" value={layer.solo.y} suffix="%" onChange={(y) => setLayer({ solo: { ...layer.solo!, y } } as Partial<TextLayer>)} />
                      </div>
                    )}
                  </Group>
                  <TextStyleProps s={layer.style} set={setStyle} />
                </>
              )}

              {layer.kind === "shape" && (
                <Group title="Shape">
                  <Color label="Fill" value={layer.fill} onChange={(fill) => setLayer({ fill } as Partial<Layer>)} />
                  <Num label="Rounded corners" value={layer.radius} step={0.25} min={0} onChange={(radius) => setLayer({ radius } as Partial<Layer>)} />
                  <Color label="Border" value={layer.border.color} onChange={(color) => setLayer({ border: { ...layer.border, color } } as Partial<Layer>)} />
                  <Num label="Border width" value={layer.border.width} step={0.05} min={0} onChange={(width) => setLayer({ border: { ...layer.border, width } } as Partial<Layer>)} />
                </Group>
              )}

              {layer.kind === "image" && (
                <Group title="Image">
                  <div className="prop-buttons">
                    <LibraryPick label="From media library…" kind="image" onPick={(m) => setLayer({ src: libRef(m) } as Partial<Layer>)} />
                    <ImagePick label="From a file…" onPick={(src) => setLayer({ src } as Partial<Layer>)} />
                  </div>
                  <Select label="Fit" value={layer.fit} options={[{ value: "contain", label: "Show all of it" }, { value: "cover", label: "Fill the box" }]} onChange={(fit) => setLayer({ fit } as Partial<Layer>)} />
                </Group>
              )}

              {layer.kind === "ticker" && (
                <>
                  <Group title="Ticker">
                    <label className="prop prop--block">
                      <span>Text</span>
                      <textarea value={layer.text} rows={3} onChange={(e) => setLayer({ text: e.target.value } as Partial<Layer>)} />
                    </label>
                    <Num label="Speed" value={layer.speed} step={1} min={1} max={60} onChange={(speed) => setLayer({ speed } as Partial<Layer>)} />
                    <Color label="Band" value={layer.fill} onChange={(fill) => setLayer({ fill } as Partial<Layer>)} />
                  </Group>
                  <TextStyleProps s={layer.style} set={setStyle} />
                </>
              )}

              <AnimProps title="Coming on" value={layer.animIn} onChange={(animIn) => setLayer({ animIn })} />
              <AnimProps title="Going off" value={layer.animOut} onChange={(animOut) => setLayer({ animOut })} />
            </>
          )}
        </aside>
      </div>
    </div>
  );
}
