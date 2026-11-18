import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { pickThemeOpen, pickThemeSave, type ForeignDesign, type ItemCollection } from "../../api";
import { ForeignThemeImport } from "../../theme/ForeignThemeImport";
import { ThemeRules } from "../../theme/ThemeRules";
import { BibleStore } from "../../BibleStore";
import { Gallery } from "../../theme/Gallery";
import { recolorToBrand } from "../../theme/brand";
import { api, type ChapterView, type VerseHit, type MediaItem, type ProgramItem, type SongRow, type SongScope, type SongTree, type Version } from "../../api";
import { Icon } from "../../Icon";
import { MediaLibrary } from "../../MediaLibrary";
import { DRAG_TYPE, dragItem, useGoLive } from "../../dnd";
import { PopupMenu, type MenuEntry } from "../../PopupMenu";
import { printChordChart, printSongWords } from "../../print";
import { AnnouncementList, ContextMenu } from "../service/panes";
import { ThemeView } from "../../theme/ThemeView";
import { KIND_NAMES, type ThemeKind, type ThemeRow } from "../../theme/types";
import type { Deck } from "../../api";

// The library across the bottom of the window, as in EasyWorship: tabs, a folder
// tree on the left, a sortable table on the right. Choosing never transmits:
// a click previews, double-click previews, drag puts an item in the schedule.

export type LibTab = "songs" | "scripture" | "media" | "presentations" | "themes";

export { DRAG_TYPE } from "../../dnd";
const ROW_H = 28;

/** Puts text on the Windows clipboard (falls back to the older copy command). */
async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const t = document.createElement("textarea");
    t.value = text;
    document.body.appendChild(t);
    t.select();
    document.execCommand("copy");
    t.remove();
  }
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

type Sort = { col: "number" | "title" | "first_line" | "langs" | "author" | "copyright"; dir: 1 | -1 };

const LANG_NAMES: Record<string, string> = { en: "English", fat: "Fante", tw: "Twi", ak: "Akan", gaa: "Ga", ga: "Ga", ee: "Ewe", dag: "Dagbani", gur: "Farefare", ha: "Hausa", fr: "French", yo: "Yoruba" };
const langNames = (l: string[]) => l.map((x) => LANG_NAMES[x] ?? x.toUpperCase()).join(" + ");

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
  const live = useGoLive();
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
    const k = (r: SongRow) => (sort.col === "number" ? r.sort_key : sort.col === "langs" ? langNames(r.langs) : (r[sort.col] ?? "").toLowerCase());
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
          {head("langs", "Languages")}
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
                <span className={r.langs.length > 1 ? "lt-langs lt-langs--two" : "lt-langs"}>{langNames(r.langs)}</span>
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
        // EasyWorship's song menu, then this app's extras
        <PopupMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          entries={(() => {
            const row = menu.row!;
            const by = (col: Sort["col"], label: string): MenuEntry => ({ label, checked: sort?.col === col, run: () => setSort({ col, dir: 1 }) });
            return [
              { label: "&New Song...", run: onNew },
              { label: "Find lyrics &online...", run: () => window.dispatchEvent(new CustomEvent("wp-lyrics")) },
              { label: "&Edit Song...", run: () => onEdit(row.id) },
              "-",
              {
                label: "&Delete",
                run: () => {
                  if (confirm(`Delete “${row.title}” from the library? A backup is made first.`)) void act(() => api.backupNow().then(() => api.deleteSong(row.id)), `“${row.title}” deleted.`);
                },
              },
              "-",
              // the schedule always shows the library's current words, so nothing to update
              { label: "&Update items in Schedule", disabled: true },
              "-",
              {
                label: "Sor&t by",
                sub: [by("number", "&Number"), by("title", "&Title"), by("first_line", "&First line"), by("author", "&Author"), by("copyright", "&Copyright"), by("langs", "&Languages"), "-", { label: "&Unsorted", checked: !sort, run: () => setSort(null) }],
              },
              { label: "&Refresh", run: () => setBump((x) => x + 1) },
              "-",
              { label: "Pri&nt", sub: [{ label: "&Words (every language)…", run: () => void printSongWords(row.id) }, { label: "&Chord chart…", run: () => void printChordChart(row.id) }] },
              "-",
              { label: "&Preview", run: () => onPreview(item(row)) },
              { label: "&Go live", run: () => live(item(row)) },
              { label: "Add to &schedule", run: () => onAdd(item(row)) },
              {
                label: "Add to my &collection",
                disabled: !tree?.mine.length,
                sub: (tree?.mine ?? []).map((f) => ({ label: f.name, run: () => void act(() => api.collectionAdd(Number(f.key.slice(5)), [row.id]), `Added to “${f.name}”.`) })),
              },
              ...(scope.kind === "mine" ? [{ label: "Remove from this collection", run: () => void act(() => api.collectionRemove(scope.id, [row.id]), "Removed from this collection.") } as MenuEntry] : []),
            ];
          })()}
        />
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

  const reloadVersions = () => api.versions().then((v) => (setVersions(v), setPrimary((p) => p || v[0]?.code || ""))).catch(() => {});
  useEffect(() => {
    void reloadVersions();
  }, []);
  // a reference opens its chapter; anything else is searched as words
  const [hits, setHits] = useState<VerseHit[] | null>(null);
  const [verseMenu, setVerseMenu] = useState<{ x: number; y: number; item: ProgramItem; text: string; version: string } | null>(null);
  // EasyWorship's View and Sort by for the verse table (remembered)
  const [cols, setColsRaw] = useState<{ ver: boolean; ref: boolean }>(() => JSON.parse(localStorage.getItem("wp-scripture-cols") || '{"ver":true,"ref":true}'));
  const setCols = (c: { ver: boolean; ref: boolean }) => (setColsRaw(c), localStorage.setItem("wp-scripture-cols", JSON.stringify(c)));
  const [hitSort, setHitSort] = useState<"reference" | "translation" | null>(null);
  const [refreshN, setRefreshN] = useState(0);
  // "My collections": Christmas readings and the like
  const [colls, setColls] = useState<ItemCollection[]>([]);
  const [store, setStore] = useState(false);
  const [openColl, setOpenColl] = useState<number | null>(null);
  const [collItems, setCollItems] = useState<ProgramItem[]>([]);
  const loadColls = () => api.itemCollections("scripture").then(setColls).catch(() => {});
  useEffect(() => void loadColls(), []);
  useEffect(() => {
    if (openColl == null) return setCollItems([]);
    api.itemCollectionItems(openColl).then(setCollItems).catch(() => {});
  }, [openColl, colls]);
  const newColl = async (item?: ProgramItem) => {
    const name = prompt("Name of the new collection (e.g. Christmas readings)");
    if (!name?.trim()) return;
    const id = await api.itemCollectionCreate("scripture", name);
    if (item) await api.itemCollectionAdd(id, item);
    void loadColls();
  };
  const [everyBible, setEveryBible] = useState(false);
  useEffect(() => {
    if (!primary) return setView(null), setHits(null);
    // nothing typed: the last chapter opened, or Genesis 1 (as EasyWorship opens)
    const asked = typed.trim() || localStorage.getItem("wp-scripture-last") || "Genesis 1";
    const t = setTimeout(
      () =>
        api
          .browseChapter(primary, asked)
          .then((v) => {
            if (typed.trim()) localStorage.setItem("wp-scripture-last", typed.trim());
            setView(v);
            setHits(null);
            setProblem(null);
            const sel = v.rows.filter((r) => r.selected).map((r) => r.verse);
            setRange(sel.length ? { from: sel[0], to: sel[sel.length - 1] } : null);
          })
          .catch((e) => {
            const words = typed.replace(/"/g, "").trim();
            if (words.length < 3 || /^\d/.test(words)) return setProblem(String(e instanceof Error ? e.message : e)), setHits(null);
            setView(null);
            api
              .searchBible(everyBible ? [] : [primary], typed)
              .then((h) => (setHits(h), setProblem(h.length ? null : `No verse contains “${words}”.`)))
              .catch((e2) => setProblem(String(e2 instanceof Error ? e2.message : e2)));
          }),
      250,
    );
    return () => clearTimeout(t);
  }, [primary, typed, everyBible, refreshN]);
  const mark = (text: string) => {
    const words = typed.replace(/"/g, "").toLowerCase().split(/\s+/).filter((w) => w.length > 1);
    if (!words.length) return text;
    const re = new RegExp(`(${words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "gi");
    return text.split(re).map((part, i) => (i % 2 ? <mark key={i}>{part}</mark> : part));
  };

  const live = useGoLive();
  const vers = second && second !== primary ? [primary, second] : [primary];
  const refFor = (from: number, to: number) => (view ? `${view.book} ${view.chapter}:${from}${to > from ? "-" + to : ""}` : "");
  const show = (from: number, to: number) => {
    setRange({ from, to });
    onPreview({ kind: "scripture", reference: refFor(from, to), versions: vers });
  };
  const goChapter = (d: number) => view && setTyped(`${view.book} ${Math.max(1, Math.min(view.chapters, view.chapter + d))}`);

  return (
    <div className={`lt ${cols.ver ? "" : "lt--hide-ver"} ${cols.ref ? "" : "lt--hide-ref"}`}>
      <aside className="lt-tree" aria-label="Bibles">
        <label className="wp-search lt-search">
          <Icon name="book" />
          <input
            ref={refInput}
            value={typed}
            placeholder="Yohane 3:16, or words: living water"
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
        <button className="lt-node lt-node--add" onClick={() => setStore(true)} title="Free Bibles from eBible.org">
          More available…
        </button>
        {store && <BibleStore onClose={() => setStore(false)} onAdded={(t) => (setStore(false), reloadVersions(), alert(`“${t}” is now in your Bibles.`))} />}
        <div className="lt-group">My collections</div>
        {colls.map((c) => (
          <button
            key={c.id}
            className={`lt-node ${openColl === c.id ? "is-current" : ""}`}
            onClick={() => setOpenColl(openColl === c.id ? null : c.id)}
            onContextMenu={(e) => {
              e.preventDefault();
              const n = prompt("Rename the collection (leave empty to delete it)", c.name);
              if (n === null) return;
              void (n.trim() ? api.itemCollectionRename(c.id, n) : confirm(`Delete “${c.name}”?`) ? api.itemCollectionDelete(c.id) : Promise.resolve()).then(() => (setOpenColl(null), loadColls()));
            }}
          >
            <span>{c.name}</span> <span className="wp-num">{c.count}</span>
          </button>
        ))}
        <button className="lt-node lt-node--add" onClick={() => void newColl()}>
          + New collection
        </button>
      </aside>
      <div className="lt-main">
        {openColl != null && (
          <div className="lt-coll">
            <div className="lt-coll__head">
              {colls.find((c) => c.id === openColl)?.name} <button className="btn-quiet" onClick={() => setOpenColl(null)}>Back to the Bible</button>
            </div>
            {collItems.length === 0 && <p className="wp-note">Empty. Right-click a verse → Add to collection.</p>}
            {collItems.map((it, i) => (
              <div
                key={i}
                className="lt-row lt-row--coll"
                draggable
                onDragStart={(e) => dragItem(e, it)}
                onClick={() => onPreview(it)}
                onDoubleClick={() => live(it)}
                onContextMenu={(e) => (e.preventDefault(), confirm("Remove this reading from the collection?") && void api.itemCollectionRemove(openColl, it).then(loadColls))}
                title="Click to preview, double-click to go live, drag to the schedule; right-click to remove"
              >
                {it.kind === "scripture" ? `${it.reference} · ${it.versions.join(" + ")}` : "title" in it ? it.title : ""}
              </div>
            ))}
          </div>
        )}
        <div className="lt-header lt-header--verses" role="row" hidden={openColl != null}>
          <span className="lt-th">Trans.</span>
          <span className="lt-th">Reference</span>
          <span className="lt-th">Scripture</span>
        </div>
        <div className="lt-body" role="grid">
          {(hitSort && hits ? [...hits].sort((x, y) => (hitSort === "translation" ? x.version.localeCompare(y.version) : 0) || 0) : hits)?.map((h) => (
            <div
              key={h.version + h.reference}
              role="row"
              className="lt-row lt-row--verses lt-row--flow"
              draggable
              onDragStart={(e) => dragItem(e, { kind: "scripture", reference: h.reference, versions: second && second !== h.version ? [h.version, second] : [h.version] })}
              onClick={() => onPreview({ kind: "scripture", reference: h.reference, versions: second && second !== h.version ? [h.version, second] : [h.version] })}
              onContextMenu={(e) => (e.preventDefault(), setVerseMenu({ x: e.clientX, y: e.clientY, text: h.text, version: h.version, item: { kind: "scripture", reference: h.reference, versions: second && second !== h.version ? [h.version, second] : [h.version] } }))}
              title="Click to preview; drag to the schedule; right-click for more"
            >
              <span>{h.version}</span>
              <span className="wp-num">
                {h.reference}
                {h.found_in && (
                  <em className="lt-via" title={`Your words were found in ${h.found_in}; the verse is shown from ${h.version}`}>
                    {" "}via {h.found_in}
                  </em>
                )}
              </span>
              <span className="lt-verse">{mark(h.text)}</span>
            </div>
          ))}
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
                onContextMenu={(e) => {
                  e.preventDefault();
                  // right-clicking inside the chosen verses uses them all; elsewhere, that verse
                  const inside = range && r.verse >= range.from && r.verse <= range.to;
                  const ref = inside ? refFor(range!.from, range!.to) : refFor(r.verse, r.verse);
                  const chosen = view.rows.filter((v) => (inside ? v.verse >= range!.from && v.verse <= range!.to : v.verse === r.verse));
                  setVerseMenu({ x: e.clientX, y: e.clientY, text: chosen.map((v) => (chosen.length > 1 ? `${v.verse} ${v.text}` : v.text)).join(" "), version: view.version, item: { kind: "scripture", reference: ref, versions: vers } });
                }}
              >
                <span>{view.version}</span>
                <span className="wp-num">{r.reference}</span>
                <span className="lt-verse">{r.text}</span>
              </div>
            </div>
          ))}
          {!view && !hits?.length && <p className="wp-empty wp-empty--inline">{problem ?? "Type a reference (a book, a chapter or verses) to see the whole chapter, or some words to find the verses that contain them."}</p>}
        </div>
        <footer className="lt-foot">
          {hits ? (
            <>
              <span className="wp-num">
                {hits.length} verse{hits.length === 1 ? "" : "s"} contain “{typed.replace(/"/g, "")}”{hits.length === 200 ? " (first 200)" : ""}
              </span>
              <label className="wp-check">
                <input type="checkbox" checked={everyBible} onChange={(e) => setEveryBible(e.target.checked)} /> Search every Bible
              </label>
              <span className="lt-note">Put words in "quotes" for an exact phrase.</span>
            </>
          ) : view ? (
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
      {verseMenu && (
        // EasyWorship's scripture menu, then this app's extras
        <PopupMenu
          x={verseMenu.x}
          y={verseMenu.y}
          onClose={() => setVerseMenu(null)}
          entries={[
            {
              label: "&View",
              sub: [
                { label: "&Translation column", checked: cols.ver, run: () => setCols({ ...cols, ver: !cols.ver }) },
                { label: "&Reference column", checked: cols.ref, run: () => setCols({ ...cols, ref: !cols.ref }) },
              ],
            },
            {
              label: "&Sort by",
              disabled: !hits,
              sub: [
                { label: "&Best match", checked: hitSort !== "translation", run: () => setHitSort(null) },
                { label: "&Translation", checked: hitSort === "translation", run: () => setHitSort("translation") },
              ],
            },
            { label: "&Refresh", run: () => setRefreshN((n) => n + 1) },
            "-",
            {
              label: "Add to co&llection",
              sub: [
                ...colls.map((c) => ({ label: c.name, run: () => void api.itemCollectionAdd(c.id, verseMenu.item).then(loadColls) })),
                ...(colls.length ? ["-" as const] : []),
                { label: "New collection…", run: () => void newColl(verseMenu.item) },
              ],
            },
            { label: "&Copy Scripture to Clipboard", run: () => void copyText(`${verseMenu.text}\n${(verseMenu.item as { reference: string }).reference} (${verseMenu.version})`) },
            "-",
            { label: "&Preview", run: () => onPreview(verseMenu.item) },
            { label: "&Go live", run: () => live(verseMenu.item) },
            { label: "Add to sc&hedule", run: () => onAdd(verseMenu.item) },
          ]}
        />
      )}
    </div>
  );
}

// ---------- Themes ----------

const THEME_KINDS: ThemeKind[] = ["song", "scripture", "lower_third", "announcement"];
const SAMPLE = { primary: ["song text line one", "song text line two"], second: ["second language line"], reference: "John 3:16", title: "Title", section: "Verse 1" };

export type ThemeEditRequest = { id?: number; newKind?: ThemeKind };

function ThemesTab({ version, previewKind, onUseForItem, onEdit }: { version: number; previewKind: ProgramItem["kind"] | null; onUseForItem: (id: number) => void; onEdit: (req?: ThemeEditRequest) => void }) {
  const [rows, setRows] = useState<ThemeRow[]>([]);
  const [kind, setKind] = useState<ThemeKind>("song");
  const [sel, setSel] = useState<number | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; row: ThemeRow | null } | null>(null);
  const [foreign, setForeign] = useState<{ path: string; designs: ForeignDesign[] } | null>(null);
  const [rulesOpen, setRulesOpen] = useState(false);
  const [galleryOpen, setGalleryOpen] = useState(false);
  const [view, setViewRaw] = useState<"pictures" | "list">(() => (localStorage.getItem("wp-themes-view") as "pictures" | "list") || "pictures");
  const setView = (v: "pictures" | "list") => (setViewRaw(v), localStorage.setItem("wp-themes-view", v));
  const [order, setOrder] = useState<"title" | "newest">("title");
  const load = () => api.listThemes().then(setRows).catch(() => {});
  useEffect(() => void load(), [version]);
  const list = rows.filter((r) => r.kind === kind).sort((x, y) => (order === "newest" ? y.id - x.id : x.name.localeCompare(y.name, undefined, { numeric: true })));
  // EasyWorship's wording: "New Song Theme...", "Set As Default Presentation Theme"
  const kindName = ({ song: "Song", scripture: "Scripture", lower_third: "Lower Third", announcement: "Presentation" } as Record<string, string>)[kind] ?? KIND_NAMES[kind] ?? kind;
  const menuEntries = (r: ThemeRow | null): MenuEntry[] => [
    { label: `&New ${kindName} Theme...`, run: () => onEdit({ newKind: kind }) },
    { label: `&Edit ${kindName} Theme...`, disabled: !r, run: () => r && onEdit({ id: r.id }) },
    "-",
    {
      label: "&Import Theme File...",
      run: async () => {
        const path = await pickThemeOpen();
        if (!path) return;
        try {
          // EasyWorship and ProPresenter themes go through the review of their designs
          if (/\.(ewtx|protheme)$/i.test(path)) return setForeign({ path, designs: await api.foreignThemeReview(path) });
          const t = await api.importTheme(path);
          setNote(`Imported “${t.name}”.`);
          load();
        } catch (e) {
          setNote(String(e instanceof Error ? e.message : e));
        }
      },
    },
    {
      label: "E&xport Theme File...",
      disabled: !r,
      run: async () => {
        if (!r) return;
        const path = await pickThemeSave(r.name);
        if (!path) return;
        try {
          await api.exportTheme(r.id, path);
          setNote(`Exported “${r.name}”.`);
        } catch (e) {
          setNote(String(e instanceof Error ? e.message : e));
        }
      },
    },
    "-",
    {
      label: `Se&t As Default ${kindName} Theme`,
      checked: !!r?.is_default,
      disabled: !r,
      run: () => r && void api.setDefaultTheme(r.kind, r.id).then(() => (setNote(`“${r.name}” is now used for every ${kindName.toLowerCase()}.`), load())),
    },
    "-",
    {
      label: "&Delete",
      disabled: !r || r.is_default,
      run: () => {
        if (!r) return;
        if (confirm(`Delete the theme “${r.name}”?`)) void api.deleteTheme(r.id).then(() => (setNote(`“${r.name}” deleted.`), load()), (e) => setNote(String(e instanceof Error ? e.message : e)));
      },
    },
    "-",
    // items use the library's themes directly, so nothing to update
    { label: "&Update Themes in Schedule", disabled: true },
    "-",
    {
      label: "&View",
      sub: [
        { label: "&Pictures", checked: view === "pictures", run: () => setView("pictures") },
        { label: "&List", checked: view === "list", run: () => setView("list") },
      ],
    },
    {
      label: "&Sort by",
      sub: [
        { label: "&Title", checked: order === "title", run: () => setOrder("title") },
        { label: "&Newest", checked: order === "newest", run: () => setOrder("newest") },
      ],
    },
    { label: "&Refresh", run: load },
    "-",
    { label: "Add from &Gallery...", run: () => setGalleryOpen(true) },
    { label: "Theme Ru&les...", run: () => setRulesOpen(true) },
    {
      label: "Re-colour to &Brand",
      disabled: !r,
      run: () => {
        if (!r || !confirm(`Re-colour “${r.name}” with the brand kit? Its colours and fonts will follow Settings → Brand kit.`)) return;
        void api.saveTheme(r.id, recolorToBrand(r.theme)).then(() => (setNote(`“${r.name}” now uses the brand kit.`), load()), (e) => setNote(String(e instanceof Error ? e.message : e)));
      },
    },
    { label: "D&uplicate", disabled: !r, run: () => r && void api.duplicateTheme(r.id).then((id) => (setNote(`Copied “${r.name}”.`), load(), onEdit({ id }))) },
  ];
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
        <div
          className={`lt-themes ${view === "list" ? "lt-themes--list" : ""}`}
          onContextMenu={(e) => {
            if ((e.target as HTMLElement).closest(".lt-theme")) return;
            e.preventDefault();
            setMenu({ x: e.clientX, y: e.clientY, row: null });
          }}
        >
          {list.map((r) => (
            <button
              key={r.id}
              className={`lt-theme ${sel === r.id ? "is-selected" : ""}`}
              onClick={() => setSel(r.id)}
              onDoubleClick={() => onEdit({ id: r.id })}
              onContextMenu={(e) => (e.preventDefault(), setSel(r.id), setMenu({ x: e.clientX, y: e.clientY, row: r }))}
              title="Double-click to edit; right-click for more"
            >
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
          <button className="wp-btn wp-btn--quiet" disabled={!chosen} onClick={() => chosen && onEdit({ id: chosen.id })}>
            Edit…
          </button>
          <button className="wp-btn wp-btn--quiet" onClick={() => onEdit({ newKind: kind })}>
            New {kindName.toLowerCase()} theme
          </button>
          <button className="wp-btn wp-btn--quiet" title="Ready-made themes in your brand colours" onClick={() => setGalleryOpen(true)}>
            Gallery…
          </button>
          <button className="wp-btn wp-btn--quiet" title="Choose themes automatically by hymnbook, language, service or Bible" onClick={() => setRulesOpen(true)}>
            Rules…
          </button>
        </footer>
      </div>
      {menu && <PopupMenu x={menu.x} y={menu.y} entries={menuEntries(menu.row)} onClose={() => setMenu(null)} />}
      {galleryOpen && <Gallery onClose={() => setGalleryOpen(false)} onAdded={(n) => (setNote(`Added “${n}”.`), load())} />}
      {rulesOpen && <ThemeRules onClose={(saved) => (setRulesOpen(false), saved && setNote("Theme rules saved."))} />}
      {foreign && (
        <ForeignThemeImport
          path={foreign.path}
          designs={foreign.designs}
          onCancel={() => setForeign(null)}
          onDone={(text) => (setForeign(null), setNote(text), load())}
        />
      )}
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
  onEditThemes: (req?: ThemeEditRequest) => void;
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
