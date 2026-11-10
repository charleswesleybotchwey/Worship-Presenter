import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, isDesktop, onLive, type Broadcast, type Display, type LangMode, type Mode, type ProgramItem, type Slide, type SongHit, type Version } from "./api";
import { SlideView } from "./SlideView";
import { Icon } from "./Icon";

interface Loaded {
  item: ProgramItem;
  slides: Slide[];
  caption: string | null;
}

const LANG_NAMES: Record<string, string> = { en: "English", fat: "Fante", tw: "Twi", gaa: "Ga", ee: "Ewe" };

function itemTitle(i: ProgramItem) {
  return i.kind === "song" ? i.title : `${i.reference}  ${i.versions.join(" + ")}`;
}

function isTyping(el: EventTarget | null) {
  const t = el as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}

export function Control() {
  const [live, setLive] = useState<Broadcast | null>(null);
  const [schedule, setSchedule] = useState<ProgramItem[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [preview, setPreview] = useState<Loaded | null>(null);
  const [previewIdx, setPreviewIdx] = useState(0);
  const [tab, setTab] = useState<"songs" | "scripture">("songs");
  const [error, setError] = useState<string | null>(null);
  const [outputsOpen, setOutputsOpen] = useState(false);
  const songSearch = useRef<HTMLInputElement>(null);
  const refInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let stop = () => {};
    onLive(setLive).then((u) => (stop = u));
    return () => stop();
  }, []);

  const run = useCallback(async <T,>(p: Promise<T>) => {
    try {
      const r = await p;
      setError(null);
      return r;
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
      return undefined;
    }
  }, []);

  const load = useCallback(
    async (item: ProgramItem) => {
      if (item.kind === "song") {
        const slides = await run(api.previewSong(item.song_id, item.title));
        if (slides) setPreview({ item, slides, caption: item.title });
      } else {
        const r = await run(api.previewScripture(item.reference, item.versions));
        if (r) setPreview({ item, slides: r[0], caption: r[1] });
      }
      setPreviewIdx(0);
    },
    [run],
  );

  const goLive = useCallback(
    async (slideIdx = previewIdx) => {
      if (preview) await run(api.goLive(preview.item, slideIdx));
    },
    [preview, previewIdx, run],
  );

  const setMode = (m: Mode) => run(api.setMode(m));
  const state = live?.state;
  const liveSlides = live?.slides ?? [];

  const stepSchedule = useCallback(
    async (d: number) => {
      if (!schedule.length) return;
      const next = Math.max(0, Math.min(schedule.length - 1, (selected ?? -1) + d));
      setSelected(next);
      await load(schedule[next]);
    },
    [schedule, selected, load],
  );

  // Keyboard model (spec section 6): presentation keys only when no field has focus;
  // emergency keys and search shortcuts work everywhere.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const k = e.key;
      if (e.ctrlKey && e.shiftKey && k.toLowerCase() === "b") return e.preventDefault(), void setMode("blank");
      if (e.ctrlKey && e.shiftKey && k.toLowerCase() === "l") return e.preventDefault(), void setMode("logo");
      if (e.ctrlKey && k.toLowerCase() === "f") return e.preventDefault(), setTab("songs"), setTimeout(() => songSearch.current?.focus());
      if (e.ctrlKey && k.toLowerCase() === "g") return e.preventDefault(), setTab("scripture"), setTimeout(() => refInput.current?.focus());
      // Go live is F5 only, and works from anywhere because F5 never types text.
      // Enter belongs to whatever button or field has focus and must never put something on screen.
      if (k === "F5") return e.preventDefault(), void goLive();
      if (isTyping(e.target) || outputsOpen) {
        if (k === "Escape") (e.target as HTMLElement).blur?.(), setOutputsOpen(false);
        return;
      }
      if (k === " " || k === "ArrowRight" || k === "ArrowDown") return e.preventDefault(), void run(api.step(1));
      if (k === "ArrowLeft" || k === "ArrowUp") return e.preventDefault(), void run(api.step(-1));
      if (k === "PageDown") return e.preventDefault(), void stepSchedule(1);
      if (k === "PageUp") return e.preventDefault(), void stepSchedule(-1);

      if (k === "Escape") return void setMode("blank");
      if (k.toLowerCase() === "l") return void setMode("logo");
      if (k.toLowerCase() === "c") return void setMode("clear");
      if (e.ctrlKey && k.toLowerCase() === "z") return void run(api.undoGoLive());
      if (/^[1-9]$/.test(k) && liveSlides.length) {
        const occ = Number(k) - 1;
        const idx = liveSlides.findIndex((s) => s.occurrence === occ);
        if (idx >= 0) void run(api.show(idx));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [goLive, liveSlides, outputsOpen, run, stepSchedule]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="topbar__brand">Worship Presenter</div>
        <button className="chip" onClick={() => setOutputsOpen((v) => !v)} aria-expanded={outputsOpen}>
          <Icon name="monitor" /> Outputs
        </button>
        <div className="topbar__spacer" />
        <div className="modes" role="group" aria-label="Screen">
          <ModeButton label="Blank" hint="Esc" active={state?.mode === "blank"} onClick={() => setMode("blank")} />
          <ModeButton label="Logo" hint="L" active={state?.mode === "logo"} onClick={() => setMode("logo")} />
          <ModeButton label="Clear" hint="C" active={state?.mode === "clear"} onClick={() => setMode("clear")} />
        </div>
        <button className="btn-golive" onClick={() => goLive()} disabled={!preview}>
          Go live <kbd>F5</kbd>
        </button>
      </header>
      {outputsOpen && <Outputs onClose={() => setOutputsOpen(false)} />}

      <main className="panes">
        <SchedulePane
          items={schedule}
          selected={selected}
          liveItem={live?.item ?? null}
          onSelect={(i) => {
            setSelected(i);
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
        />

        <section className="pane pane--library" aria-label="Library">
          <div className="tabs" role="tablist">
            <button role="tab" aria-selected={tab === "songs"} onClick={() => setTab("songs")}>
              Songs
            </button>
            <button role="tab" aria-selected={tab === "scripture"} onClick={() => setTab("scripture")}>
              Scripture
            </button>
          </div>
          {tab === "songs" ? (
            <SongLibrary
              inputRef={songSearch}
              onPreview={(h) => void load({ kind: "song", song_id: h.id, title: h.title })}
              onAdd={(h) => setSchedule((s) => [...s, { kind: "song", song_id: h.id, title: h.title }])}
            />
          ) : (
            <ScriptureLibrary
              inputRef={refInput}
              onPreview={(reference, versions) => void load({ kind: "scripture", reference, versions })}
              onAdd={(reference, versions) => setSchedule((s) => [...s, { kind: "scripture", reference, versions }])}
            />
          )}
        </section>

        <section className="pane pane--preview" aria-label="Preview">
          <div className="pane__head">
            <span className="tally tally--preview">Preview</span>
            <span className="pane__title">{preview ? itemTitle(preview.item) : "Nothing selected"}</span>
          </div>
          <div className="stage">
            <SlideView slide={preview?.slides[previewIdx]} caption={preview?.item.kind === "scripture" ? preview.caption : null} />
          </div>
          <SlideGrid
            slides={preview?.slides ?? []}
            current={previewIdx}
            onPick={setPreviewIdx}
            onActivate={(i) => void goLive(i)}
            empty="Pick a song or type a scripture reference. Double-click a slide to send it live."
          />
        </section>

        <section className={`pane pane--live ${state?.mode === "live" ? "is-on-air" : ""}`} aria-label="Live">
          <div className="pane__head">
            <span className="tally tally--live">Live</span>
            <span className="pane__title">{live?.item ? itemTitle(live.item) : "Nothing live"}</span>
            <span className="pane__state">{modeText(state?.mode)}</span>
          </div>
          <div className="stage stage--live">
            <SlideView slide={liveSlides[state?.slide_idx ?? 0]} caption={live?.item?.kind === "scripture" ? live.caption : null} blank={state?.mode !== "live"} />
          </div>
          <div className="live-tools">
            <Segmented
              label="Lines"
              value={String(state?.lines_per_slide ?? 2)}
              options={["1", "2", "3", "4"].map((v) => ({ value: v, label: v }))}
              onChange={(v) => void run(api.setLayout(Number(v)))}
            />
            {live?.item?.kind === "song" && liveSlides.some((s) => s.second.length > 0 || state?.lang_mode === "second") && (
              <Segmented
                label="Language"
                value={state?.lang_mode ?? "both"}
                options={[
                  { value: "primary", label: LANG_NAMES.en },
                  { value: "both", label: "Both" },
                  { value: "second", label: LANG_NAMES.fat },
                ]}
                onChange={(v) => void run(api.setLayout(undefined, v as LangMode))}
              />
            )}
            <div className="topbar__spacer" />
            <button className="btn-quiet" onClick={() => void run(api.step(-1))} disabled={!liveSlides.length}>
              Back
            </button>
            <button className="btn-quiet" onClick={() => void run(api.step(1))} disabled={!liveSlides.length}>
              Next
            </button>
          </div>
          <SlideGrid slides={liveSlides} current={state?.slide_idx ?? -1} onPick={(i) => void run(api.show(i))} onActivate={(i) => void run(api.show(i))} empty="Live is empty. Send something from Preview." live />
        </section>
      </main>

      <footer className="status">
        <span>{liveSlides.length ? `Slide ${(state?.slide_idx ?? 0) + 1} of ${liveSlides.length}` : "No live item"}</span>
        <span>{isDesktop ? "Library: default profile" : "Browser preview with sample data"}</span>
        {error && (
          <span className="status__error" role="alert">
            {error}
          </span>
        )}
        <span className="status__right">Space next, ← back, Esc blank, Ctrl+Shift+B blank from anywhere</span>
      </footer>
    </div>
  );
}

function modeText(m?: Mode) {
  return m === "live" ? "On screen" : m === "blank" ? "Blank" : m === "logo" ? "Logo" : m === "clear" ? "Text cleared" : "";
}

function ModeButton({ label, hint, active, onClick }: { label: string; hint: string; active?: boolean; onClick: () => void }) {
  return (
    <button className={`mode ${active ? "is-active" : ""}`} aria-pressed={!!active} onClick={onClick}>
      {label} <kbd>{hint}</kbd>
    </button>
  );
}

function Segmented({ label, value, options, onChange }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (v: string) => void }) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      <span className="segmented__label">{label}</span>
      {options.map((o) => (
        <button key={o.value} role="radio" aria-checked={o.value === value} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

function SlideGrid({ slides, current, onPick, onActivate, empty, live }: { slides: Slide[]; current: number; onPick: (i: number) => void; onActivate: (i: number) => void; empty: string; live?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>(".thumb.is-current")?.scrollIntoView({ block: "nearest" });
  }, [current]);
  if (!slides.length) return <p className="empty">{empty}</p>;
  let lastLabel = "";
  return (
    <div className="grid" ref={ref}>
      {slides.map((s, i) => {
        const showLabel = `${s.label}#${s.occurrence}` !== lastLabel;
        lastLabel = `${s.label}#${s.occurrence}`;
        return (
          <button
            key={i}
            className={`thumb ${i === current ? "is-current" : ""} ${live ? "thumb--live" : ""}`}
            onClick={() => onPick(i)}
            onDoubleClick={() => onActivate(i)}
            aria-current={i === current}
          >
            <SlideView slide={s} />
            <span className="thumb__label">{showLabel ? s.label : ""}</span>
          </button>
        );
      })}
    </div>
  );
}

function SchedulePane({ items, selected, liveItem, onSelect, onRemove, onMove }: { items: ProgramItem[]; selected: number | null; liveItem: ProgramItem | null; onSelect: (i: number) => void; onRemove: (i: number) => void; onMove: (from: number, to: number) => void }) {
  const drag = useRef<number | null>(null);
  const same = (a: ProgramItem, b: ProgramItem | null) => !!b && JSON.stringify(a) === JSON.stringify(b);
  return (
    <section className="pane pane--schedule" aria-label="Schedule">
      <div className="pane__head">
        <span className="pane__title">Service order</span>
      </div>
      {items.length === 0 ? (
        <p className="empty">Add songs and readings from the library with the + button.</p>
      ) : (
        <ol className="schedule">
          {items.map((it, i) => (
            <li
              key={i}
              draggable
              onDragStart={() => (drag.current = i)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={() => drag.current !== null && onMove(drag.current, i)}
              className={`${selected === i ? "is-selected" : ""} ${same(it, liveItem) ? "is-live" : ""}`}
            >
              <button className="schedule__item" onClick={() => onSelect(i)}>
                <Icon name={it.kind === "song" ? "note" : "book"} />
                <span>{itemTitle(it)}</span>
              </button>
              <button className="icon-btn" aria-label={`Remove ${itemTitle(it)}`} onClick={() => onRemove(i)}>
                <Icon name="close" />
              </button>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function SongLibrary({ inputRef, onPreview, onAdd }: { inputRef: React.RefObject<HTMLInputElement>; onPreview: (h: SongHit) => void; onAdd: (h: SongHit) => void }) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SongHit[]>([]);
  const [active, setActive] = useState(0);
  useEffect(() => {
    const t = setTimeout(() => api.searchSongs(q).then(setHits).catch(() => setHits([])), 120);
    return () => clearTimeout(t);
  }, [q]);
  return (
    <>
      <label className="search">
        <Icon name="search" />
        <input
          ref={inputRef}
          value={q}
          placeholder="Number, title or a line: MHB 50, “thousand tongues”"
          onChange={(e) => (setQ(e.target.value), setActive(0))}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") e.preventDefault(), setActive((a) => Math.min(hits.length - 1, a + 1));
            if (e.key === "ArrowUp") e.preventDefault(), setActive((a) => Math.max(0, a - 1));
            if (e.key === "Enter" && hits[active]) e.preventDefault(), onPreview(hits[active]);
          }}
        />
      </label>
      <ul className="results">
        {hits.map((h, i) => (
          <li key={h.id} className={i === active ? "is-active" : ""}>
            <button className="result" onClick={() => (setActive(i), onPreview(h))}>
              <span className="result__num">{h.numbers[0] ?? ""}</span>
              <span className="result__title">{h.first_line ?? h.title}</span>
              <span className="result__meta">{h.title !== h.numbers[0] ? h.title : h.author ?? ""}</span>
            </button>
            <button className="icon-btn" aria-label={`Add ${h.title} to the service`} onClick={() => onAdd(h)}>
              <Icon name="plus" />
            </button>
          </li>
        ))}
        {q && !hits.length && <p className="empty">No songs match “{q}”. Try the hymn number or a different line.</p>}
      </ul>
    </>
  );
}

function ScriptureLibrary({ inputRef, onPreview, onAdd }: { inputRef: React.RefObject<HTMLInputElement>; onPreview: (r: string, v: string[]) => void; onAdd: (r: string, v: string[]) => void }) {
  const [ref, setRef] = useState("");
  const [versions, setVersions] = useState<Version[]>([]);
  const [chosen, setChosen] = useState<string[]>([]);
  useEffect(() => {
    api.versions().then((v) => {
      setVersions(v);
      setChosen((c) => (c.length ? c : v.slice(0, 1).map((x) => x.code)));
    });
  }, []);
  const toggle = (code: string) =>
    setChosen((c) => (c.includes(code) ? c.filter((x) => x !== code) : c.length >= 2 ? [c[0], code] : [...c, code]));
  const ready = ref.trim().length > 2 && chosen.length > 0;
  return (
    <div className="scripture">
      <label className="search">
        <Icon name="book" />
        <input
          ref={inputRef}
          value={ref}
          placeholder="Jn 3:16, Yohane 3:16-18, Nnwom 23"
          onChange={(e) => setRef(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && ready && (e.preventDefault(), onPreview(ref, chosen))}
        />
      </label>
      <p className="hint">Pick one version, or two to show them together.</p>
      <ul className="versions">
        {versions.map((v) => (
          <li key={v.id}>
            <button className={`version ${chosen.includes(v.code) ? "is-on" : ""}`} aria-pressed={chosen.includes(v.code)} onClick={() => toggle(v.code)}>
              <span className="version__code">{v.code}</span>
              <span className="version__name">{v.name}</span>
              {chosen.indexOf(v.code) >= 0 && <span className="version__order">{chosen.indexOf(v.code) === 0 ? "Top" : "Under"}</span>}
            </button>
          </li>
        ))}
        {!versions.length && <p className="empty">No Bibles yet. Import an EasyWorship (.ewb) or Bible Show (.bib) file.</p>}
      </ul>
      <div className="row">
        <button className="btn" disabled={!ready} onClick={() => onPreview(ref, chosen)}>
          Preview
        </button>
        <button className="btn-quiet" disabled={!ready} onClick={() => onAdd(ref, chosen)}>
          Add to service
        </button>
      </div>
    </div>
  );
}

function Outputs({ onClose }: { onClose: () => void }) {
  const [displays, setDisplays] = useState<Display[]>([]);
  const [main, setMain] = useState<number | null>(null);
  const [lower, setLower] = useState<number | null>(null);
  useEffect(() => {
    api.displays().then((d) => {
      setDisplays(d);
      const other = d.find((x) => !x.primary);
      setMain(other ? other.index : null);
    });
  }, []);
  const roles = useMemo(
    () => [
      { role: "main", name: "Main projector", value: main, set: setMain },
      { role: "alternate", name: "Lower third for the stream", value: lower, set: setLower },
    ],
    [main, lower],
  );
  return (
    <div className="popover" role="dialog" aria-label="Outputs">
      {roles.map((r) => (
        <div className="output-row" key={r.role}>
          <span className="output-row__name">{r.name}</span>
          <select value={r.value ?? ""} onChange={(e) => r.set(e.target.value === "" ? null : Number(e.target.value))}>
            <option value="">In a window on this screen</option>
            {displays.map((d) => (
              <option key={d.index} value={d.index}>
                {d.name} ({d.width}×{d.height}){d.primary ? ", this screen" : ""}
              </option>
            ))}
          </select>
          <button className="btn" onClick={() => api.openOutput(r.role, r.value)}>
            Open
          </button>
          <button className="btn-quiet" onClick={() => api.closeOutput(r.role)}>
            Close
          </button>
        </div>
      ))}
      <div className="row row--end">
        <button className="btn-quiet" onClick={onClose}>
          Done
        </button>
      </div>
    </div>
  );
}
