import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, type ChapterView, type MediaItem, type ProgramItem, type SongRow, type SongScope, type SongTree, type Version } from "../../api";
import { Icon } from "../../Icon";
import { MediaLibrary } from "../../MediaLibrary";
import { AnnouncementList, ContextMenu } from "../service/panes";
import { ThemeView } from "../../theme/ThemeView";
import { KIND_NAMES, type ThemeKind, type ThemeRow } from "../../theme/types";
import type { Deck } from "../../api";

// The library across the bottom of the window, as in EasyWorship: tabs, a folder
// tree on the left, a sortable table on the right. Choosing never transmits:
// a click previews, double-click previews, drag puts an item in the schedule.

export type LibTab = "songs" | "scripture" | "media" | "presentations" | "themes";

export const DRAG_TYPE = "application/x-wp-item";
const ROW_H = 28;

function dragItem(e: React.DragEvent, item: ProgramItem) {
  e.dataTransfer.setData(DRAG_TYPE, JSON.stringify(item));
  e.dataTransfer.effectAllowed = "copy";
}

/** Draws only the rows in view, so 5,000 songs scroll smoothly. */
function useWindow(count: number) {
  const ref = useRef<HTMLDivElement>(null);
  const [range, setRange] = useState({ from: 0, to: 60 });
  const update = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    const from = Math.max(0, Math.floor(el.scrollTop / ROW_H) - 10);
    setRange({ from, to: Math.min(count, from + Math.ceil(el.clientHeight / ROW_H) + 20) });
  }, [count]);
  useEffect(update, [update]);
  return { ref, range, onScroll: update };
}

// ---------- Songs ----------

type Sort = { col: "number" | "title" | "first_line" | "author" | "copyright"; dir: 1 | -1 };

function SongsTab({
  searchRef,
  version,
  onPreview,
  onAdd,
  onEdit,
  onNew,
}: {
  searchRef: React.RefObject<HTMLInputElement>;
  version: number;
  onPreview: (i: ProgramItem) => void;
  onAdd: (i: ProgramItem) => void;
  onEdit: (id: number) => void;
  onNew: () => void;
}) {
  const [tree, setTree] = useState<SongTree | null>(null);
  const [scope, setScope] = useState<SongScope>({ kind: "all" });
  const [open, setOpen] = useState({ collections: true, mine: true });
  const [q, setQ] = useState("");
  const [rows, setRows] = useState<SongRow[]>([]);
  const [sort, setSort] = useState<Sort | null>(null);
  const [sel, setSel] = useState<number | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; row?: SongRow; folder?: { id: number; name: string } } | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [bump, setBump] = useState(0);

  useEffect(() => {
    api.songTree().then(setTree).catch(() => {});
  }, [version, bump]);
  useEffect(() => {
    const t = setTimeout(() => api.browseSongs(scope, q).then(setRows).catch(() => setRows([])), 120);
    return () => clearTimeout(t);
  }, [scope, q, version, bump]);

  const sorted = useMemo(() => {
    if (!sort) return rows;
    const k = (r: SongRow) => (sort.col === "number" ? r.sort_key : (r[sort.col] ?? "").toLowerCase());
    return [...rows].sort((a, b) => (k(a) < k(b) ? -sort.dir : k(a) > k(b) ? sort.dir : 0));
  }, [rows, sort]);
  const win = useWindow(sorted.length);

  const item = (r: SongRow): ProgramItem => ({ kind: "song", song_id: r.id, title: r.title });
  const scopeKey = scope.kind === "all" ? "all" : scope.kind === "hymnal" ? `hymnal:${scope.code}` : `mine:${scope.id}`;
  const pick = (key: string) => setScope(key === "all" ? { kind: "all" } : key.startsWith("hymnal:") ? { kind: "hymnal", code: key.slice(7) } : { kind: "mine", id: Number(key.slice(5)) });
  const head = (col: Sort["col"], label: string) => (
    <button className="lt-th" onClick={() => setSort(sort?.col === col ? (sort.dir === 1 ? { col, dir: -1 } : null) : { col, dir: 1 })} aria-sort={sort?.col === col ? (sort.dir === 1 ? "ascending" : "descending") : "none"}>
      {label} {sort?.col === col ? (sort.dir === 1 ? "▲" : "▼") : ""}
    </button>
  );
  const act = async (f: () => Promise<unknown>, ok?: string) => {
    try {
      await f();
      setNote(ok ?? null);
      setBump((b) => b + 1);
    } catch (e) {
      setNote(String(e instanceof Error ? e.message : e));
    }
  };

  return (
    <div className="lt">
      <aside className="lt-tree" aria-label="Song folders">
        <label className="wp-search lt-search">
          <Icon name="search" />
          <input
            ref={searchRef}
            value={q}
            placeholder="Search any field"
            aria-label="Search songs"
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && sorted[0]) e.preventDefault(), setSel(sorted[0].id), onPreview(item(sorted[0]));
            }}
          />
        </label>
        <div className="lt-group">Songs</div>
        <button className={`lt-node ${scopeKey === "all" ? "is-selected" : ""}`} onClick={() => pick("all")}>
          <Icon name="note" /> All songs <span className="lt-count">{tree?.all ?? ""}</span>
        </button>
        <button className="lt-group lt-group--btn" onClick={() => setOpen({ ...open, collections: !open.collections })} aria-expanded={open.collections}>
          {open.collections ? "▾" : "▸"} Collections
        </button>
        {open.collections &&
          tree?.collections.map((f) => (
            <button key={f.key} className={`lt-node lt-node--sub ${scopeKey === f.key ? "is-selected" : ""}`} onClick={() => pick(f.key)}>
              {f.name} <span className="lt-count">{f.count}</span>
            </button>
          ))}
        <button className="lt-group lt-group--btn" onClick={() => setOpen({ ...open, mine: !open.mine })} aria-expanded={open.mine}>
          {open.mine ? "▾" : "▸"} My collections
        </button>
        {open.mine && (
          <>
            {tree?.mine.map((f) => (
              <button
                key={f.key}
                className={`lt-node lt-node--sub ${scopeKey === f.key ? "is-selected" : ""}`}
                onClick={() => pick(f.key)}
                onContextMenu={(e) => (e.preventDefault(), setMenu({ x: e.clientX, y: e.clientY, folder: { id: Number(f.key.slice(5)), name: f.name } }))}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => {
                  const raw = e.dataTransfer.getData(DRAG_TYPE);
                  const it = raw ? (JSON.parse(raw) as ProgramItem) : null;
                  if (it?.kind === "song") void act(() => api.collectionAdd(Number(f.key.slice(5)), [it.song_id]), `Added to “${f.name}”.`);
                }}
              >
                {f.name} <span className="lt-count">{f.count}</span>
              </button>
            ))}
            <button
              className="lt-node lt-node--sub lt-node--add"
              onClick={() => {
                const name = prompt("Name for the new collection", "Sunday favourites");
                if (name?.trim()) void act(() => api.collectionCreate(name), `Collection “${name.trim()}” made. Right-click songs to add them, or drag them onto it.`);
              }}
            >
              + New collection
            </button>
          </>
        )}
      </aside>
      <div className="lt-main">
        <div className="lt-header lt-header--songs" role="row">
          {head("number", "Number")}
          {head("title", "Title")}
          {head("first_line", "First line")}
          {head("author", "Author")}
          {head("copyright", "Copyright")}
        </div>
        <div className="lt-body" ref={win.ref} onScroll={win.onScroll} role="grid" aria-rowcount={sorted.length}>
          <div style={{ height: sorted.length * ROW_H, position: "relative" }}>
            {sorted.slice(win.range.from, win.range.to).map((r, k) => (
              <div
                key={r.id}
                role="row"
                aria-selected={sel === r.id}
                className={`lt-row lt-row--songs ${sel === r.id ? "is-selected" : ""}`}
                style={{ top: (win.range.from + k) * ROW_H }}
                draggable
                onDragStart={(e) => dragItem(e, item(r))}
                onClick={() => (setSel(r.id), onPreview(item(r)))}
                onDoubleClick={() => onPreview(item(r))}
                onContextMenu={(e) => (e.preventDefault(), setSel(r.id), setMenu({ x: e.clientX, y: e.clientY, row: r }))}
                title={r.first_line ?? r.title}
              >
                <span className="wp-num">{r.number ?? ""}</span>
                <span>{r.title}</span>
                <span>{r.first_line ?? ""}</span>
                <span>{r.author ?? ""}</span>
                <span>{r.copyright ?? ""}</span>
              </div>
            ))}
          </div>
          {!sorted.length && <p className="wp-empty wp-empty--inline">{q ? `No songs match “${q}”.` : "No songs here yet."}</p>}
        </div>
        <footer className="lt-foot">
          <span className="wp-num">
            {sorted.length.toLocaleString()} song{sorted.length === 1 ? "" : "s"}
          </span>
          {note && <span className="lt-note">{note}</span>}
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--quiet" onClick={onNew}>
            + New song
          </button>
        </footer>
      </div>
      {menu?.row && (
        <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)}>
          <button role="menuitem" onClick={() => (setMenu(null), onPreview(item(menu.row!)))}>
            Preview
          </button>
          <button role="menuitem" onClick={() => (setMenu(null), onAdd(item(menu.row!)))}>
            Add to schedule
          </button>
          <button role="menuitem" onClick={() => (setMenu(null), onEdit(menu.row!.id))}>
            Edit song
          </button>
          {tree?.mine.map((f) => (
            <button key={f.key} role="menuitem" onClick={() => (setMenu(null), void act(() => api.collectionAdd(Number(f.key.slice(5)), [menu.row!.id]), `Added to “${f.name}”.`))}>
              Add to “{f.name}”
            </button>
          ))}
          {scope.kind === "mine" && (
            <button role="menuitem" onClick={() => (setMenu(null), void act(() => api.collectionRemove(scope.id, [menu.row!.id]), "Removed from this collection."))}>
              Remove from this collection
            </button>
          )}
        </ContextMenu>
      )}
      {menu?.folder && (
        <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)}>
          <button
            role="menuitem"
            onClick={() => {
              setMenu(null);
              const n = prompt("New name", menu.folder!.name);
              if (n?.trim()) void act(() => api.collectionRename(menu.folder!.id, n));
            }}
          >
            Rename
          </button>
          <button
            role="menuitem"
            className="btn-danger"
            onClick={() => {
              setMenu(null);
              if (confirm(`Delete the collection “${menu.folder!.name}”? The songs stay in the library.`)) void act(() => api.collectionDelete(menu.folder!.id).then(() => setScope({ kind: "all" })));
            }}
          >
            Delete collection
          </button>
        </ContextMenu>
      )}
    </div>
  );
}

// ---------- Scriptures ----------

function ScripturesTab({ refInput, onPreview, onAdd }: { refInput: React.RefObject<HTMLInputElement>; onPreview: (i: ProgramItem) => void; onAdd: (i: ProgramItem) => void }) {
  const [versions, setVersions] = useState<Version[]>([]);
  const [primary, setPrimary] = useState<string>("");
  const [second, setSecond] = useState<string>("");
  const [typed, setTyped] = useState("");
  const [view, setView] = useState<ChapterView | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [range, setRange] = useState<{ from: number; to: number } | null>(null);
  const anchor = useRef<number | null>(null);

  useEffect(() => {
    api.versions().then((v) => (setVersions(v), setPrimary((p) => p || v[0]?.code || ""))).catch(() => {});
  }, []);
  useEffect(() => {
    if (!primary || !typed.trim()) return setView(null);
    const t = setTimeout(
      () =>
        api
          .browseChapter(primary, typed)
          .then((v) => {
            setView(v);
            setProblem(null);
            const sel = v.rows.filter((r) => r.selected).map((r) => r.verse);
            setRange(sel.length ? { from: sel[0], to: sel[sel.length - 1] } : null);
          })
          .catch((e) => setProblem(String(e instanceof Error ? e.message : e))),
      150,
    );
    return () => clearTimeout(t);
  }, [primary, typed]);

  const vers = second && second !== primary ? [primary, second] : [primary];
  const refFor = (from: number, to: number) => (view ? `${view.book} ${view.chapter}:${from}${to > from ? "-" + to : ""}` : "");
  const show = (from: number, to: number) => {
    setRange({ from, to });
    onPreview({ kind: "scripture", reference: refFor(from, to), versions: vers });
  };
  const goChapter = (d: number) => view && setTyped(`${view.book} ${Math.max(1, Math.min(view.chapters, view.chapter + d))}`);

  return (
    <div className="lt">
      <aside className="lt-tree" aria-label="Bibles">
        <label className="wp-search lt-search">
          <Icon name="book" />
          <input
            ref={refInput}
            value={typed}
            placeholder="Genesis 1:1, Yohane 3:16"
            aria-label="Scripture reference"
            onChange={(e) => setTyped(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && view) {
                e.preventDefault();
                const sel = view.rows.filter((r) => r.selected);
                if (sel.length) show(sel[0].verse, sel[sel.length - 1].verse);
                else onPreview({ kind: "scripture", reference: typed, versions: vers });
              }
            }}
          />
        </label>
        <div className="lt-group">Scriptures</div>
        {versions.map((v) => (
          <button key={v.code} className={`lt-node ${primary === v.code ? "is-selected" : ""}`} onClick={() => setPrimary(v.code)} title={v.name}>
            <Icon name="book" /> {v.code} <span className="lt-count lt-count--name">{v.name}</span>
          </button>
        ))}
        {!versions.length && <p className="wp-note">No Bibles yet. Import one with Ctrl+K, Import.</p>}
        <label className="lt-second">
          <span>Show with</span>
          <select value={second} onChange={(e) => setSecond(e.target.value)}>
            <option value="">Only {primary}</option>
            {versions
              .filter((v) => v.code !== primary)
              .map((v) => (
                <option key={v.code} value={v.code}>
                  {v.code}
                </option>
              ))}
          </select>
        </label>
      </aside>
      <div className="lt-main">
        <div className="lt-header lt-header--verses" role="row">
          <span className="lt-th">Trans.</span>
          <span className="lt-th">Reference</span>
          <span className="lt-th">Scripture</span>
        </div>
        <div className="lt-body" role="grid">
          {view?.rows.map((r) => (
            <div key={r.verse}>
              {r.heading && <div className="lt-heading">{r.heading}</div>}
              <div
                role="row"
                aria-selected={!!range && r.verse >= range.from && r.verse <= range.to}
                className={`lt-row lt-row--verses lt-row--flow ${range && r.verse >= range.from && r.verse <= range.to ? "is-selected" : ""}`}
                draggable
                onDragStart={(e) => dragItem(e, { kind: "scripture", reference: refFor(r.verse, r.verse), versions: vers })}
                onClick={(e) => {
                  if (e.shiftKey && anchor.current != null) show(Math.min(anchor.current, r.verse), Math.max(anchor.current, r.verse));
                  else (anchor.current = r.verse), show(r.verse, r.verse);
                }}
                onContextMenu={(e) => (e.preventDefault(), range && onAdd({ kind: "scripture", reference: refFor(range.from, range.to), versions: vers }))}
              >
                <span>{view.version}</span>
                <span className="wp-num">{r.reference}</span>
                <span className="lt-verse">{r.text}</span>
              </div>
            </div>
          ))}
          {!view && <p className="wp-empty wp-empty--inline">{problem ?? "Type a reference on the left: a book, a chapter or verses. The whole chapter appears here."}</p>}
        </div>
        <footer className="lt-foot">
          {view ? (
            <>
              <button className="wp-icon-btn" aria-label="Previous chapter" onClick={() => goChapter(-1)} disabled={view.chapter <= 1}>
                ‹
              </button>
              <span className="wp-num">
                {view.book} {view.chapter} · {view.rows.length} verses
              </span>
              <button className="wp-icon-btn" aria-label="Next chapter" onClick={() => goChapter(1)} disabled={view.chapter >= view.chapters}>
                ›
              </button>
              <span className="lt-note">Click a verse to preview it; shift-click for several. Right-click adds to the schedule.</span>
            </>
          ) : (
            <span />
          )}
          <span className="wp-spacer" />
          {range && view && (
            <button className="wp-btn wp-btn--quiet" onClick={() => onAdd({ kind: "scripture", reference: refFor(range.from, range.to), versions: vers })}>
              Add {refFor(range.from, range.to)} to schedule
            </button>
          )}
        </footer>
      </div>
    </div>
  );
}

// ---------- Themes ----------

const THEME_KINDS: ThemeKind[] = ["song", "scripture", "lower_third", "announcement"];
const SAMPLE = { primary: ["song text line one", "song text line two"], second: ["second language line"], reference: "John 3:16", title: "Title" };

function ThemesTab({ version, previewKind, onUseForItem, onEdit }: { version: number; previewKind: ProgramItem["kind"] | null; onUseForItem: (id: number) => void; onEdit: () => void }) {
  const [rows, setRows] = useState<ThemeRow[]>([]);
  const [kind, setKind] = useState<ThemeKind>("song");
  const [sel, setSel] = useState<number | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const load = () => api.listThemes().then(setRows).catch(() => {});
  useEffect(() => void load(), [version]);
  const list = rows.filter((r) => r.kind === kind);
  const chosen = rows.find((r) => r.id === sel);
  const fits = chosen && previewKind && (chosen.kind === previewKind || (chosen.kind === "song" && previewKind === "song"));
  return (
    <div className="lt">
      <aside className="lt-tree" aria-label="Theme kinds">
        <div className="lt-group">Themes</div>
        {THEME_KINDS.map((k) => (
          <button key={k} className={`lt-node ${kind === k ? "is-selected" : ""}`} onClick={() => setKind(k)}>
            <Icon name="theme" /> {KIND_NAMES[k] ?? k} <span className="lt-count">{rows.filter((r) => r.kind === k).length}</span>
          </button>
        ))}
      </aside>
      <div className="lt-main">
        <div className="lt-themes">
          {list.map((r) => (
            <button key={r.id} className={`lt-theme ${sel === r.id ? "is-selected" : ""}`} onClick={() => setSel(r.id)} onDoubleClick={onEdit} title="Double-click to edit">
              <div className="lt-theme__pic">
                <ThemeView theme={r.theme} data={SAMPLE} still />
              </div>
              <span>
                {r.name}
                {r.is_default ? " · in use" : ""}
              </span>
            </button>
          ))}
        </div>
        <footer className="lt-foot">
          <span className="wp-num">
            {list.length} theme{list.length === 1 ? "" : "s"}
          </span>
          {note && <span className="lt-note">{note}</span>}
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--quiet" disabled={!fits} title={fits ? "Use this theme for the item in Preview only" : "Preview a song or reading of this kind first"} onClick={() => chosen && (onUseForItem(chosen.id), setNote(`Preview item uses “${chosen.name}”.`))}>
            Use for this item
          </button>
          <button
            className="wp-btn wp-btn--quiet"
            disabled={!chosen || chosen.is_default}
            onClick={() => chosen && api.setDefaultTheme(chosen.kind, chosen.id).then(() => (setNote(`“${chosen.name}” is now used for every ${KIND_NAMES[chosen.kind]?.toLowerCase() ?? chosen.kind}.`), load()))}
          >
            Use for all
          </button>
          <button className="wp-btn wp-btn--quiet" onClick={onEdit}>
            Edit themes…
          </button>
        </footer>
      </div>
    </div>
  );
}

// ---------- the dock ----------

export function LibraryDock(p: {
  tab: LibTab;
  setTab: (t: LibTab) => void;
  version: number;
  songSearch: React.RefObject<HTMLInputElement>;
  refInput: React.RefObject<HTMLInputElement>;
  previewKind: ProgramItem["kind"] | null;
  onPreview: (i: ProgramItem) => void;
  onAdd: (i: ProgramItem) => void;
  onEditSong: (id: number) => void;
  onNewSong: () => void;
  onMediaPreview: (m: MediaItem) => void;
  onEditDeck: (id: number) => void;
  onNewDeck: (t: Deck | null) => void;
  onUseTheme: (id: number) => void;
  onEditThemes: () => void;
  extraTab?: { label: string; node: React.ReactNode };
}) {
  const tabs: [LibTab, string][] = [
    ["songs", "Songs"],
    ["scripture", "Scriptures"],
    ["media", "Media"],
    ["presentations", "Presentations"],
    ["themes", "Themes"],
  ];
  const [extra, setExtra] = useState(false);
  return (
    <section className="wp-pane lib-dock" aria-label="Library" data-region="library">
      <div className="lib-tabs" role="tablist" aria-label="Library">
        {p.extraTab && (
          <button role="tab" aria-selected={extra} onClick={() => setExtra(true)}>
            {p.extraTab.label}
          </button>
        )}
        {tabs.map(([t, label]) => (
          <button key={t} role="tab" aria-selected={!extra && p.tab === t} onClick={() => (setExtra(false), p.setTab(t))}>
            {label}
          </button>
        ))}
      </div>
      <div className="lib-body">
        {extra && p.extraTab ? (
          p.extraTab.node
        ) : p.tab === "songs" ? (
          <SongsTab searchRef={p.songSearch} version={p.version} onPreview={p.onPreview} onAdd={p.onAdd} onEdit={p.onEditSong} onNew={p.onNewSong} />
        ) : p.tab === "scripture" ? (
          <ScripturesTab refInput={p.refInput} onPreview={p.onPreview} onAdd={p.onAdd} />
        ) : p.tab === "media" ? (
          <div className="lt lt--single">
            <MediaLibrary version={p.version} onPreview={p.onMediaPreview} onAdd={(m) => p.onAdd({ kind: "media", media_id: m.id, title: m.title })} />
          </div>
        ) : p.tab === "presentations" ? (
          <div className="lt lt--single">
            <AnnouncementList
              version={p.version}
              onPreview={(d) => p.onPreview({ kind: "announcement", presentation_id: d.id, title: d.title })}
              onAdd={(d) => p.onAdd({ kind: "announcement", presentation_id: d.id, title: d.title })}
              onEdit={p.onEditDeck}
              onNew={p.onNewDeck}
            />
          </div>
        ) : (
          <ThemesTab version={p.version} previewKind={p.previewKind} onUseForItem={p.onUseTheme} onEdit={p.onEditThemes} />
        )}
      </div>
    </section>
  );
}
