import { dragItem, ItemMenu } from "../../dnd";
import { useEffect, useRef, useState } from "react";
import { api, pickPresentation, type Display, type MediaItem, type ProgramItem, type Slide, type SongHit, type Version } from "../../api";
import { SlideView } from "../../SlideView";
import { Icon } from "../../Icon";
import { importPresentation, methodText } from "../library/importPresentation";

export function itemTitle(i: ProgramItem) {
  return i.kind === "scripture" ? `${i.reference} · ${i.versions.join(" + ")}` : i.title;
}

export const sameItem = (a: ProgramItem | null | undefined, b: ProgramItem | null | undefined) =>
  !!a &&
  !!b &&
  a.kind === b.kind &&
  (a.kind === "song"
    ? a.song_id === (b as typeof a).song_id
    : a.kind === "scripture"
      ? a.reference === (b as typeof a).reference
      : a.kind === "announcement"
        ? a.presentation_id === (b as typeof a).presentation_id
        : a.kind === "header"
          ? a.title === (b as typeof a).title
          : a.kind === "web"
            ? a.url === (b as typeof a).url
            : a.media_id === (b as typeof a).media_id);

/** The per-item look, for items that can have one. */
export const themeIdOf = (i: ProgramItem | null | undefined) => (i && (i.kind === "song" || i.kind === "scripture") ? i.theme_id ?? null : null);

const glyph = (k: ProgramItem["kind"]) => (k === "song" ? "note" : k === "media" || k === "web" ? "image" : k === "announcement" ? "slides" : "book");

// ---------- Schedule (the sidebar) ----------

export function SchedulePane({
  items,
  selected,
  liveItem,
  onSelect,
  onActivate,
  onHeading,
  onWebPage,
  onThemeDrop,
  onRemove,
  onMove,
  onAddFromLibrary,
  onMenu,
  dateLabel,
  onDropItem,
}: {
  items: ProgramItem[];
  selected: number | null;
  liveItem: ProgramItem | null;
  onSelect: (i: number) => void;
  /** Double-click: straight to the screens (as in EasyWorship). */
  onActivate?: (i: number) => void;
  /** Add a heading below item i (null: at the end), or rename heading i. */
  onHeading?: (i: number | null, rename: boolean) => void;
  /** Add a web page below item i (null: at the end). */
  onWebPage?: (i: number | null) => void;
  /** A theme dropped on item i (from the Themes tab). */
  onThemeDrop?: (i: number, themeId: number) => void;
  onRemove: (i: number) => void;
  onMove: (from: number, to: number) => void;
  onAddFromLibrary: () => void;
  onMenu: () => void;
  dateLabel: string;
  /** An item dragged from the library, dropped before `at` (or at the end). */
  onDropItem?: (item: ProgramItem, at: number | null) => void;
}) {
  const drag = useRef<number | null>(null);
  const dropFrom = (e: React.DragEvent, at: number | null) => {
    const raw = e.dataTransfer.getData("application/x-wp-item");
    if (raw && onDropItem) {
      e.preventDefault();
      e.stopPropagation();
      // one item, or a multiple selection, kept in order at the drop point
      const v = JSON.parse(raw) as ProgramItem | ProgramItem[];
      (Array.isArray(v) ? v : [v]).forEach((it, k) => onDropItem(it, at == null ? null : at + k));
      return true;
    }
    return false;
  };
  return (
    <section
      className="wp-pane wp-pane--schedule"
      aria-label="Schedule"
      data-region="schedule"
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => void dropFrom(e, null)}
    >
      <header className="wp-pane__head">
        <h2>Schedule</h2>
        {items.length > 0 && <span className="wp-count">{items.length}</span>}
        <span className="wp-spacer" />
        <button className="wp-icon-btn" title="Add from library (Ctrl+F)" aria-label="Add from library" onClick={onAddFromLibrary}>
          <Icon name="plus" />
        </button>
        {onWebPage && (
          <button className="wp-icon-btn" title="Add a web page (shown over the main screen when live)" aria-label="Add a web page" onClick={() => onWebPage(selected ?? null)}>
            🌐
          </button>
        )}
        {onHeading && (
          <button className="wp-icon-btn wp-heading-btn" title="Add a heading (Praise & Worship, Sermon…)" aria-label="Add a heading" onClick={() => onHeading(selected ?? null, false)}>
            H
          </button>
        )}
        <button className="wp-icon-btn" title="Open, start or rename a service" aria-label="Service orders" onClick={onMenu}>
          <Icon name="more" />
        </button>
      </header>
      {dateLabel && <div className="wp-subhead">{dateLabel}</div>}
      {items.length === 0 ? (
        <div className="wp-empty">
          <p>No items in this service</p>
          <button className="wp-btn" onClick={onAddFromLibrary}>
            Add from library
          </button>
        </div>
      ) : (
        <ol className="wp-rows wp-rows--schedule" role="listbox" aria-label="Service order">
          {items.map((it, i) => {
            const isLive = sameItem(it, liveItem);
            // items are numbered without the headings
            const num = items.slice(0, i + 1).filter((x) => x.kind !== "header").length;
            return (
              <li
                key={i}
                role="option"
                aria-selected={selected === i}
                tabIndex={selected === i || (selected == null && i === 0) ? 0 : -1}
                className={`wp-row ${it.kind === "header" ? "wp-row--header" : ""} ${selected === i ? "is-selected" : ""}`}
                draggable
                title={onActivate ? `${itemTitle(it)} (double-click to go live)` : itemTitle(it)}
                onDragStart={() => (drag.current = i)}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => {
                  // a theme dragged from the Themes tab: this item takes it
                  const themes = e.dataTransfer.getData("application/x-wp-themes");
                  if (themes && onThemeDrop) {
                    e.preventDefault();
                    e.stopPropagation();
                    const [id] = JSON.parse(themes) as number[];
                    if (id != null) onThemeDrop(i, id);
                    return;
                  }
                  if (dropFrom(e, i)) return;
                  if (drag.current !== null) onMove(drag.current, i);
                }}
                onClick={() => onSelect(i)}
                onDoubleClick={it.kind === "header" ? () => onHeading?.(i, true) : onActivate ? () => onActivate(i) : undefined}
                onKeyDown={(e) => {
                  if (e.altKey && e.key === "ArrowUp" && i > 0) return e.preventDefault(), onMove(i, i - 1);
                  if (e.altKey && e.key === "ArrowDown" && i < items.length - 1) return e.preventDefault(), onMove(i, i + 1);
                  if (e.key === "ArrowDown") return e.preventDefault(), (e.currentTarget.nextElementSibling as HTMLElement | null)?.focus();
                  if (e.key === "ArrowUp") return e.preventDefault(), (e.currentTarget.previousElementSibling as HTMLElement | null)?.focus();
                  if (e.key === "Enter" || e.key === " ") return e.preventDefault(), onSelect(i);
                  if (e.key === "Delete") return e.preventDefault(), onRemove(i);
                }}
              >
                <span className="wp-row__num">{it.kind === "header" ? "" : String(num).padStart(2, "0")}</span>
                {it.kind !== "header" && <Icon name={glyph(it.kind)} />}
                <span className="wp-row__title">{itemTitle(it)}</span>
                {it.kind === "song" && it.lyrics && <span className="wp-row__meta">Edited words</span>}
                {isLive && (
                  <span className="wp-live-tag">
                    <Icon name="speaker" /> Live
                  </span>
                )}
                <button className="wp-icon-btn wp-row__x" aria-label={`Remove ${itemTitle(it)}`} title="Remove (Delete)" onClick={(e) => (e.stopPropagation(), onRemove(i))}>
                  <Icon name="close" />
                </button>
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}

// ---------- Library: songs ----------

export function SongList({
  inputRef,
  version,
  onEdit,
  onNew,
  onPreview,
  onAdd,
}: {
  inputRef: React.RefObject<HTMLInputElement>;
  version: number;
  onEdit: (id: number) => void;
  onNew: () => void;
  onPreview: (h: SongHit) => void;
  onAdd: (h: SongHit) => void;
}) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SongHit[]>([]);
  const [active, setActive] = useState(0);
  const [menu, setMenu] = useState<{ id: number; x: number; y: number } | null>(null);
  useEffect(() => {
    const t = setTimeout(() => api.searchSongs(q).then(setHits).catch(() => setHits([])), 120);
    return () => clearTimeout(t);
  }, [q, version]);
  const hit = hits.find((h) => h.id === menu?.id);
  return (
    <>
      <div className="wp-toolbar">
        <label className="wp-search">
          <Icon name="search" />
          <input
            ref={inputRef}
            value={q}
            placeholder="Search songs: MHB 50, a title or a line"
            aria-label="Search songs"
            onChange={(e) => (setQ(e.target.value), setActive(0))}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") e.preventDefault(), setActive((a) => Math.min(hits.length - 1, a + 1));
              if (e.key === "ArrowUp") e.preventDefault(), setActive((a) => Math.max(0, a - 1));
              if (e.key === "Enter" && hits[active]) e.preventDefault(), onPreview(hits[active]);
            }}
          />
          {q && (
            <button className="wp-icon-btn" aria-label="Clear search" onClick={() => setQ("")}>
              <Icon name="close" />
            </button>
          )}
        </label>
        <button className="wp-btn wp-btn--quiet" onClick={onNew} title="Add a song by hand">
          New
        </button>
      </div>
      <ul className="wp-rows" role="listbox" aria-label="Songs">
        {hits.map((h, i) => (
          <li
            key={h.id}
            role="option"
            aria-selected={i === active}
            className={`wp-row wp-row--lib ${i === active ? "is-selected" : ""}`}
            title={h.title}
            onClick={() => (setActive(i), onPreview(h))}
            onContextMenu={(e) => (e.preventDefault(), setMenu({ id: h.id, x: e.clientX, y: e.clientY }))}
          >
            <span className="wp-row__num wp-row__num--wide">{h.numbers[0] ?? ""}</span>
            <span className="wp-row__title">{h.first_line ?? h.title}</span>
            <span className="wp-row__meta">{h.numbers[0] && h.title !== h.numbers[0] ? h.title : h.author ?? ""}</span>
            <button className="wp-icon-btn wp-row__x" aria-label={`Edit ${h.title}`} title="Edit" onClick={(e) => (e.stopPropagation(), onEdit(h.id))}>
              <Icon name="edit" />
            </button>
            <button className="wp-icon-btn wp-row__x" aria-label={`Add ${h.title} to the service`} title="Add to the service" onClick={(e) => (e.stopPropagation(), onAdd(h))}>
              <Icon name="plus" />
            </button>
          </li>
        ))}
        {!hits.length && (
          <li className="wp-empty wp-empty--inline">
            {q ? (
              <>
                <p>No songs match “{q}”.</p>
                <button className="wp-btn wp-btn--quiet" onClick={() => setQ("")}>
                  Clear search
                </button>
              </>
            ) : (
              <p>No songs yet. Import them with Ctrl+K, Import.</p>
            )}
          </li>
        )}
      </ul>
      {menu && hit && (
        <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)}>
          <button role="menuitem" onClick={() => (setMenu(null), onPreview(hit))}>
            Preview
          </button>
          <button role="menuitem" onClick={() => (setMenu(null), onAdd(hit))}>
            Add to the service
          </button>
          <button role="menuitem" onClick={() => (setMenu(null), onEdit(hit.id))}>
            Edit song
          </button>
        </ContextMenu>
      )}
    </>
  );
}

export function ContextMenu({ x, y, onClose, children }: { x: number; y: number; onClose: () => void; children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  useEffect(() => {
    // flip inside the window instead of clipping
    const r = ref.current?.getBoundingClientRect();
    if (r) setPos({ x: Math.min(x, window.innerWidth - r.width - 8), y: Math.min(y, window.innerHeight - r.height - 8) });
    (ref.current?.querySelector("button") as HTMLElement | null)?.focus();
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && onClose();
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [x, y, onClose]);
  return (
    <div
      ref={ref}
      className="wp-menu"
      role="menu"
      style={{ left: pos.x, top: pos.y }}
      onKeyDown={(e) => {
        const items = [...(ref.current?.querySelectorAll("button") ?? [])] as HTMLElement[];
        const i = items.indexOf(document.activeElement as HTMLElement);
        if (e.key === "ArrowDown") e.preventDefault(), items[(i + 1) % items.length]?.focus();
        if (e.key === "ArrowUp") e.preventDefault(), items[(i - 1 + items.length) % items.length]?.focus();
        if (e.key === "Escape") e.preventDefault(), e.stopPropagation(), onClose();
      }}
    >
      {children}
    </div>
  );
}

// ---------- Library: scripture ----------

export function ScripturePicker({
  inputRef,
  onPreview,
  onAdd,
}: {
  inputRef: React.RefObject<HTMLInputElement>;
  onPreview: (r: string, v: string[]) => void;
  onAdd: (r: string, v: string[]) => void;
}) {
  const [ref, setRef] = useState("");
  const [versions, setVersions] = useState<Version[]>([]);
  const [chosen, setChosen] = useState<string[]>([]);
  useEffect(() => {
    api.versions().then((v) => {
      setVersions(v);
      setChosen((c) => (c.length ? c : v.slice(0, 1).map((x) => x.code)));
    });
  }, []);
  const toggle = (code: string) => setChosen((c) => (c.includes(code) ? c.filter((x) => x !== code) : c.length >= 2 ? [c[0], code] : [...c, code]));
  const ready = ref.trim().length > 2 && chosen.length > 0;
  return (
    <>
      <div className="wp-toolbar">
        <label className="wp-search">
          <Icon name="book" />
          <input
            ref={inputRef}
            value={ref}
            placeholder="Reference: Jn 3:16, Yohane 3:16-18"
            aria-label="Scripture reference"
            onChange={(e) => setRef(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && ready && (e.preventDefault(), onPreview(ref, chosen))}
          />
        </label>
      </div>
      <div className="wp-subhead">Versions: one, or two to show together</div>
      <ul className="wp-rows" role="listbox" aria-multiselectable="true" aria-label="Bible versions">
        {versions.map((v) => {
          const pos = chosen.indexOf(v.code);
          return (
            <li key={v.id} role="option" aria-selected={pos >= 0} className={`wp-row wp-row--lib ${pos >= 0 ? "is-selected" : ""}`} onClick={() => toggle(v.code)}>
              <span className="wp-row__num wp-row__num--wide">{v.code}</span>
              <span className="wp-row__title">{v.name}</span>
              <span className="wp-row__meta">{pos === 0 ? "First" : pos === 1 ? "Second" : ""}</span>
            </li>
          );
        })}
        {!versions.length && (
          <li className="wp-empty wp-empty--inline">
            <p>No Bibles yet. Import an EasyWorship (.ewb) or Bible Show (.bib) file with Ctrl+K, Import.</p>
          </li>
        )}
      </ul>
      <div className="wp-actions">
        <button className="wp-btn" disabled={!ready} onClick={() => onPreview(ref, chosen)}>
          Preview
        </button>
        <button className="wp-btn wp-btn--quiet" disabled={!ready} onClick={() => onAdd(ref, chosen)}>
          Add to service
        </button>
      </div>
    </>
  );
}

// ---------- slide grid ----------

export function SlideGrid({
  slides,
  kind,
  media,
  themeId,
  current,
  onPick,
  onActivate,
  empty,
  live,
  confirmed = true,
}: {
  slides: Slide[];
  kind?: ProgramItem["kind"];
  media?: MediaItem | null;
  themeId?: number | null;
  current: number;
  onPick: (i: number) => void;
  /** Double-click: Preview sends the slide live (same rules as Go live). */
  onActivate?: (i: number) => void;
  empty: string;
  live?: boolean;
  /** Main has acknowledged this slide; only then is it marked Live. */
  confirmed?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>(".wp-thumb.is-current")?.scrollIntoView({ block: "nearest" });
  }, [current]);
  if (!slides.length) return <p className="wp-empty wp-empty--inline">{empty}</p>;
  let last = "";
  return (
    <div className="wp-grid" ref={ref} role="listbox" aria-label={live ? "Live slides: choosing one changes the screens" : "Preview slides"}>
      {slides.map((s, i) => {
        const tag = `${s.label}#${s.occurrence}`;
        const head = tag !== last;
        last = tag;
        return (
          <button
            key={i}
            role="option"
            aria-selected={i === current}
            className={`wp-thumb ${i === current ? "is-current" : ""} ${live ? "wp-thumb--live" : ""}`}
            onClick={() => onPick(i)}
            onDoubleClick={onActivate ? () => onActivate(i) : undefined}
            title={live ? "Show this slide on the screens" : onActivate ? "Click to stage, double-click to go live" : "Stage this slide"}
          >
            <span className="wp-thumb__bar">
              <span>{head ? s.label : ""}</span>
              <span className="wp-thumb__pos">{i + 1}</span>
              {live && i === current && confirmed && (
                <span className="wp-live-tag">
                  <Icon name="speaker" /> Live
                </span>
              )}
            </span>
            <SlideView slide={s} position={{ first: i === 0, last: i === slides.length - 1 }} kind={kind} media={media} themeId={themeId} still />
          </button>
        );
      })}
    </div>
  );
}

// ---------- output setup ----------

export function OutputSetup({ onClose, mainLive }: { onClose: () => void; mainLive: boolean }) {
  const [displays, setDisplays] = useState<Display[]>([]);
  const [pick, setPick] = useState<Record<string, number | null>>({ main: null, alternate: null, foldback: null });
  useEffect(() => {
    api.displays().then((d) => {
      setDisplays(d);
      const other = d.find((x) => !x.primary);
      setPick((p) => ({ ...p, main: other ? other.index : null }));
    });
  }, []);
  const roles = [
    { role: "main", name: "Main", detail: "Projector or LED wall the congregation reads" },
    { role: "alternate", name: "Lower third", detail: "For the stream: keyed in OBS or vMix" },
    { role: "foldback", name: "Stage", detail: "Pulpit and band: current, next, clock, notes" },
  ];
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog" role="dialog" aria-modal="true" aria-label="Output setup">
        <header className="wp-dialog__head">
          <h2>Output setup</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose} autoFocus>
            Done
          </button>
        </header>
        {roles.map((r) => (
          <div className="wp-setting" key={r.role}>
            <div>
              <div className="wp-setting__name">{r.name}</div>
              <div className="wp-setting__hint">{r.detail}</div>
            </div>
            <select aria-label={`${r.name} destination`} value={pick[r.role] ?? ""} onChange={(e) => setPick({ ...pick, [r.role]: e.target.value === "" ? null : Number(e.target.value) })}>
              <option value="">Window on this screen</option>
              {displays.map((d) => (
                <option key={d.index} value={d.index}>
                  {d.name} ({d.width}×{d.height}){d.primary ? ", this screen" : ""}
                </option>
              ))}
            </select>
            <button className="wp-btn" onClick={() => api.openOutput(r.role, pick[r.role])}>
              Open
            </button>
            <button
              className="wp-btn wp-btn--quiet"
              onClick={() => {
                if (r.role === "main" && mainLive && !confirm("Main is showing the service. Identify puts a test pattern on the screen the congregation can see for 4 seconds. Continue?")) return;
                void api.identifyOutput(r.role);
              }}
            >
              Identify
            </button>
            <button className="wp-btn wp-btn--quiet" onClick={() => api.closeOutput(r.role)}>
              Close
            </button>
          </div>
        ))}
        <p className="wp-note">A lost public display is never moved onto the screen you are working on.</p>
      </div>
    </div>
  );
}

// ---------- Library: announcements ----------

export function AnnouncementList({
  version,
  onPreview,
  onAdd,
  onEdit,
  onNew,
}: {
  version: number;
  onPreview: (d: import("../../api").DeckSummary) => void;
  onAdd: (d: import("../../api").DeckSummary) => void;
  onEdit: (id: number) => void;
  onNew: (template: import("../../api").Deck | null) => void;
}) {
  const [decks, setDecks] = useState<import("../../api").DeckSummary[]>([]);
  const [templates, setTemplates] = useState<import("../../api").Deck[]>([]);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [deckMenu, setDeckMenu] = useState<{ d: import("../../api").DeckSummary; x: number; y: number } | null>(null);
  const [active, setActive] = useState<number | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [report, setReport] = useState<{ text: string; lines: string[]; bad?: boolean } | null>(null);
  const [bump, setBump] = useState(0);
  useEffect(() => {
    api.listAnnouncements().then(setDecks).catch(() => {});
    api.announcementTemplates().then(setTemplates).catch(() => {});
  }, [version, bump]);
  const doImport = async () => {
    const path = await pickPresentation();
    if (!path) return;
    try {
      const r = await importPresentation(path, setBusy);
      setReport({ text: `“${r.title}” imported. ${methodText(r.method)}`, lines: r.warnings });
      setBump((b) => b + 1);
      setActive(r.deckId);
      onPreview({ id: r.deckId, title: r.title, slides: 0, looping: false });
    } catch (e) {
      setReport({ text: String(e instanceof Error ? e.message : e), lines: [], bad: true });
    } finally {
      setBusy(null);
    }
  };
  return (
    <>
      <div className="wp-toolbar">
        <span className="wp-subhead wp-subhead--flush">Announcements, and PowerPoint or PDF presentations</span>
        <span className="wp-spacer" />
        <button className="wp-btn" disabled={!!busy} onClick={doImport}>
          {busy ? busy : "Import PowerPoint or PDF…"}
        </button>
        <button className="wp-btn wp-btn--quiet" onClick={(e) => setMenu({ x: e.clientX, y: e.clientY + 8 })} aria-haspopup="menu">
          New <Icon name="more" />
        </button>
      </div>
      {report && (
        <div className={`wp-banner ${report.bad ? "wp-banner--bad" : ""}`} role="status">
          <span>
            {report.text}
            {report.lines.map((l) => (
              <span key={l} className="import-line">
                {l}
              </span>
            ))}
          </span>
          <button className="wp-icon-btn" aria-label="Dismiss" onClick={() => setReport(null)}>
            <Icon name="close" />
          </button>
        </div>
      )}
      <ul className="wp-rows" role="listbox" aria-label="Announcements">
        {decks.map((d) => (
          <li
            key={d.id}
            role="option"
            aria-selected={active === d.id}
            className={`wp-row wp-row--lib ${active === d.id ? "is-selected" : ""}`}
            draggable
            onDragStart={(e) => dragItem(e, { kind: "announcement", presentation_id: d.id, title: d.title })}
            onContextMenu={(e) => (e.preventDefault(), setActive(d.id), setDeckMenu({ d, x: e.clientX, y: e.clientY }))}
            onClick={() => (setActive(d.id), onPreview(d))}
          >
            <Icon name="slides" />
            <span className="wp-row__title">{d.title}</span>
            <span className="wp-row__meta">
              {d.slides} slides{d.looping ? ", loops" : ""}
            </span>
            <button className="wp-icon-btn wp-row__x" aria-label={`Edit ${d.title}`} title="Edit" onClick={(e) => (e.stopPropagation(), onEdit(d.id))}>
              <Icon name="edit" />
            </button>
            <button className="wp-icon-btn wp-row__x" aria-label={`Add ${d.title} to the service`} title="Add to the service" onClick={(e) => (e.stopPropagation(), onAdd(d))}>
              <Icon name="plus" />
            </button>
          </li>
        ))}
        {!decks.length && (
          <li className="wp-empty wp-empty--inline">
            <p>No announcements yet.</p>
            <button className="wp-btn" onClick={(e) => setMenu({ x: e.clientX, y: e.clientY })}>
              New announcements
            </button>
          </li>
        )}
      </ul>
      {menu && (
        <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)}>
          <button role="menuitem" onClick={() => (setMenu(null), onNew(null))}>
            Blank
          </button>
          <button role="menuitem" onClick={() => (setMenu(null), window.dispatchEvent(new CustomEvent("wp-outline")))}>
            From a sermon outline…
          </button>
          <button role="menuitem" onClick={() => (setMenu(null), window.dispatchEvent(new CustomEvent("wp-offering")))}>
            Offering slide (MoMo, QR)…
          </button>
          {templates.map((t) => (
            <button key={t.title} role="menuitem" onClick={() => (setMenu(null), onNew(t))}>
              From the pack: {t.title}
            </button>
          ))}
        </ContextMenu>
      )}
      {deckMenu && (
        <ItemMenu
          x={deckMenu.x}
          y={deckMenu.y}
          item={{ kind: "announcement", presentation_id: deckMenu.d.id, title: deckMenu.d.title }}
          onClose={() => setDeckMenu(null)}
          onPreview={() => onPreview(deckMenu.d)}
          onAdd={() => onAdd(deckMenu.d)}
        >
          <button role="menuitem" onClick={() => (setDeckMenu(null), onEdit(deckMenu.d.id))}>
            Edit
          </button>
        </ItemMenu>
      )}
    </>
  );
}
