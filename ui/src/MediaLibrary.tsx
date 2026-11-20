import { dragItem, useGoLive } from "./dnd";
import { OnlineMedia } from "./OnlineMedia";
import { useCallback, useEffect, useMemo, useState, useRef } from "react";
import { PopupMenu, type MenuEntry } from "./PopupMenu";
import { api, pickMediaFiles, type ItemCollection, type MediaItem } from "./api";
import { makeThumb, useMediaUrl } from "./theme/media";
import type { ThemeKind } from "./theme/types";
import { Icon } from "./Icon";

// Pictures, videos and sound in the current profile. Click to preview, + to add to
// the service order, or set a picture or video as the background of a theme.

const KINDS = [
  { value: "", label: "All" },
  { value: "image", label: "Pictures" },
  { value: "video", label: "Videos" },
  { value: "audio", label: "Sound" },
] as const;

export const duration = (ms: number | null) => {
  if (ms == null) return "";
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

export function thumbSrc(m: MediaItem, url: (s: string) => string) {
  if (!m.thumb) return m.kind === "image" ? url(`media:${m.path}`) : null;
  return url(m.thumb.startsWith("data:") ? m.thumb : `media:${m.thumb}`);
}

/** Missing thumbnails are made in the background, one at a time. */
async function fillThumbs(items: MediaItem[], url: (s: string) => string) {
  for (const m of items.filter((x) => !x.thumb && x.kind !== "audio" && !x.missing)) {
    try {
      await makeThumb(m, url(`media:${m.path}`));
    } catch {
      /* a file the webview cannot decode keeps its placeholder */
    }
  }
}

export type MediaSort = "title" | "file" | "newest" | "size";
export type MediaGridControl = {
  importFiles: () => void;
  layout: "list" | "thumbs";
  setLayout: (l: "list" | "thumbs") => void;
  sort: MediaSort;
  setSort: (s: MediaSort) => void;
  refresh: () => void;
  /** The items chosen together (Ctrl/Shift+click), when the menu is on one of them. */
  selection: MediaItem[];
  clearSelection: () => void;
};

export function MediaGrid({
  kind,
  onPick,
  selectedId,
  actions,
  menuFor,
  draggableItems,
  onlyIds,
}: {
  kind?: MediaItem["kind"] | "";
  onPick: (m: MediaItem) => void;
  selectedId?: number | null;
  actions?: (m: MediaItem) => React.ReactNode;
  /** The right-click menu for an item (the media library's EasyWorship menu). */
  menuFor?: (m: MediaItem, ctl: MediaGridControl) => MenuEntry[] | Promise<MenuEntry[]>;
  /** Items can be dragged to the schedule. */
  draggableItems?: boolean;
  /** Show only these items (a media collection). */
  onlyIds?: number[] | null;
}) {
  // several items at once: Ctrl+click adds or removes one, Shift+click takes a range
  const [multi, setMulti] = useState<Set<number>>(new Set());
  const anchor = useRef<number | null>(null);
  // the item last clicked here (the library does not always say which is chosen)
  const [lastPick, setLastPick] = useState<number | null>(null);
  const current = selectedId ?? lastPick;
  const many = (m: MediaItem) => (multi.size > 1 && multi.has(m.id) ? shownRef.current.filter((x) => multi.has(x.id)) : []);
  const pickWith = (e: React.MouseEvent, m: MediaItem) => {
    const idx = shownRef.current.findIndex((x) => x.id === m.id);
    if (e.ctrlKey || e.metaKey) {
      const next = new Set(multi.size ? multi : current != null ? [current] : []);
      if (next.has(m.id)) next.delete(m.id);
      else next.add(m.id);
      anchor.current = idx;
      return setMulti(next);
    }
    if (e.shiftKey && anchor.current != null) {
      const [a, b] = [Math.min(anchor.current, idx), Math.max(anchor.current, idx)];
      return setMulti(new Set(shownRef.current.slice(a, b + 1).map((x) => x.id)));
    }
    anchor.current = idx;
    setMulti(new Set());
    setLastPick(m.id);
    if (m.missing) return setProblem(`“${m.title}” is missing from the media folder. Use Relink to point it at the file.`);
    onPick(m);
  };
  const isPicked = (m: MediaItem) => (multi.size ? multi.has(m.id) : m.id === current);
  const dragProps = (m: MediaItem) =>
    draggableItems && !m.missing
      ? {
          draggable: true,
          onDragStart: (e: React.DragEvent) => {
            const sel = many(m).filter((x) => !x.missing);
            dragItem(e, sel.length > 1 ? sel.map((x) => ({ kind: "media" as const, media_id: x.id, title: x.title })) : { kind: "media", media_id: m.id, title: m.title });
          },
        }
      : {};
  const [itemMenu, setItemMenu] = useState<{ x: number; y: number; entries: MenuEntry[] } | null>(null);
  const shownRef = useRef<MediaItem[]>([]);
  const [sort, setSort] = useState<MediaSort>(() => (localStorage.getItem("wp-media-sort") as MediaSort) || "title");
  const setSortSaved = (v: MediaSort) => (setSort(v), localStorage.setItem("wp-media-sort", v));
  const menuProps = (m: MediaItem) =>
    menuFor
      ? {
          onContextMenu: async (e: React.MouseEvent) => {
            e.preventDefault();
            const { clientX: x, clientY: y } = e;
            if (!multi.has(m.id)) setMulti(new Set());
            const entries = await menuFor(m, { importFiles, layout, setLayout: setLayoutSaved, sort, setSort: setSortSaved, refresh: () => void refresh(), selection: many(m), clearSelection: () => setMulti(new Set()) });
            setItemMenu({ x, y, entries });
          },
        }
      : {};
  const url = useMediaUrl();
  const [items, setItems] = useState<MediaItem[]>([]);
  const [filter, setFilter] = useState<string>(kind ?? "");
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  // dense list by default (design brief §5); thumbnails on request
  const [layout, setLayout] = useState<"list" | "thumbs">(() => (localStorage.getItem("wp-media-layout") as "list" | "thumbs") || "list");
  const setLayoutSaved = (l: "list" | "thumbs") => (setLayout(l), localStorage.setItem("wp-media-layout", l));

  const relink = async (m: MediaItem) => {
    const paths = await pickMediaFiles();
    if (!paths[0]) return;
    try {
      const fixed = await api.relinkMedia(m.id, paths[0]);
      await makeThumb(fixed, url(`media:${fixed.path}`)).catch(() => {});
      setProblem(null);
      await refresh();
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };

  const refresh = useCallback(async () => {
    const list = await api.listMedia((filter || null) as MediaItem["kind"] | null, q);
    setItems(list);
    if (list.some((m) => !m.thumb && m.kind !== "audio" && !m.missing)) {
      await fillThumbs(list, url);
      setItems(await api.listMedia((filter || null) as MediaItem["kind"] | null, q));
    }
  }, [filter, q, url]);

  useEffect(() => {
    const t = setTimeout(() => void refresh().catch((e) => setProblem(String(e))), 120);
    return () => clearTimeout(t);
  }, [refresh]);
  const shown = useMemo(() => {
    const file = (m: MediaItem) => m.path.split("/").pop() ?? m.path;
    const by: Record<MediaSort, (a: MediaItem, b: MediaItem) => number> = {
      title: (a, b) => a.title.localeCompare(b.title, undefined, { numeric: true }),
      file: (a, b) => file(a).localeCompare(file(b), undefined, { numeric: true }),
      newest: (a, b) => b.id - a.id,
      size: (a, b) => b.bytes - a.bytes,
    };
    return [...items].filter((m) => !onlyIds || onlyIds.includes(m.id)).sort(by[sort]);
  }, [items, sort, onlyIds]);

  const importFiles = async () => {
    const paths = await pickMediaFiles();
    if (!paths.length) return;
    setBusy(true);
    try {
      const r = await api.importMedia(paths);
      const failed = r.filter((x) => x.error);
      setProblem(failed.length ? failed.map((x) => x.error).join(" ") : null);
      await refresh();
    } finally {
      setBusy(false);
    }
  };

  shownRef.current = shown;
  return (
    <div className="media">
      <div className="media__bar">
        <label className="search">
          <Icon name="search" />
          <input value={q} placeholder="Search media" onChange={(e) => setQ(e.target.value)} />
        </label>
        <button className="btn" onClick={importFiles} disabled={busy}>
          {busy ? "Copying…" : "Import…"}
        </button>
      </div>
      <div className="media__opts">
        {kind === undefined && (
          <div className="wp-tabs" role="radiogroup" aria-label="Show">
            {KINDS.map((k) => (
              <button key={k.value} role="radio" aria-selected={filter === k.value} aria-checked={filter === k.value} onClick={() => setFilter(k.value)}>
                {k.label}
              </button>
            ))}
          </div>
        )}
        <span className="wp-spacer" />
        <div className="wp-tabs" role="radiogroup" aria-label="Layout">
          <button role="radio" aria-selected={layout === "list"} aria-checked={layout === "list"} onClick={() => setLayoutSaved("list")}>
            List
          </button>
          <button role="radio" aria-selected={layout === "thumbs"} aria-checked={layout === "thumbs"} onClick={() => setLayoutSaved("thumbs")}>
            Thumbnails
          </button>
        </div>
      </div>
      {problem && <p className="form-error">{problem}</p>}
      {layout === "list" && (
        <ul className="wp-rows" role="listbox" aria-label="Media">
          {shown.map((m) => (
            <li
              key={m.id}
              role="option"
              aria-selected={isPicked(m)}
              className={`wp-row wp-row--lib ${isPicked(m) ? "is-selected" : ""}`}
              title={m.title}
              {...dragProps(m)}
              {...menuProps(m)}
              onClick={(e) => pickWith(e, m)}
            >
              <Icon name={m.kind === "image" ? "image" : m.kind === "video" ? "video" : "speaker"} />
              <span className="wp-row__title">{m.title}</span>
              {m.missing ? (
                <>
                  <span className="wp-row__warn">File missing</span>
                  <button className="wp-btn wp-btn--quiet" onClick={(e) => (e.stopPropagation(), void relink(m))}>
                    Relink…
                  </button>
                </>
              ) : (
                <span className="wp-row__meta wp-num">{m.kind === "image" ? (m.width ? `${m.width}×${m.height}` : "Picture") : duration(m.duration_ms) || (m.kind === "video" ? "Video" : "Sound")}</span>
              )}
              {actions && !m.missing && <span className="media__rowactions">{actions(m)}</span>}
            </li>
          ))}
          {!items.length && <li className="wp-empty wp-empty--inline">{q ? `Nothing matches “${q}”.` : "No media yet. Import pictures, videos or sound; they are copied into this profile."}</li>}
        </ul>
      )}
      <ul className="media__grid" hidden={layout !== "thumbs"}>
        {layout === "thumbs" && shown.map((m) => {
          const t = thumbSrc(m, url);
          return (
            <li key={m.id} className={isPicked(m) ? "is-current" : ""} {...dragProps(m)} {...menuProps(m)}>
              <button className="media__card" onClick={(e) => pickWith(e, m)} title={m.title}>
                <span className="media__thumb">{t ? <img src={t} alt="" loading="lazy" /> : <span className="media__icon">{m.kind === "audio" ? "♪" : "▶"}</span>}</span>
                <span className="media__title">{m.title}</span>
                <span className={`media__meta ${m.missing ? "is-missing" : ""}`}>{m.missing ? "File missing" : m.kind === "image" ? "Picture" : `${m.kind === "video" ? "Video" : "Sound"} ${duration(m.duration_ms)}`}</span>
              </button>
              {actions && <div className="media__actions">{actions(m)}</div>}
            </li>
          );
        })}
        {layout === "thumbs" && !items.length && <p className="empty">{q ? `Nothing matches “${q}”.` : "No media yet. Import pictures, videos or sound; they are copied into this profile."}</p>}
      </ul>
      {itemMenu && <PopupMenu x={itemMenu.x} y={itemMenu.y} entries={itemMenu.entries} onClose={() => setItemMenu(null)} />}
    </div>
  );
}

export function MediaLibrary({ onPreview, onAdd, version }: { onPreview: (m: MediaItem) => void; onAdd: (m: MediaItem) => void; version: number }) {
  const [menu, setMenu] = useState<number | null>(null);
  const [note, setNote] = useState<{ text: string; bad?: boolean } | null>(null);
  // the library, or free pictures and videos online (Pixabay, Unsplash)
  const [online, setOnline] = useState(false);
  const [bump, setBump] = useState(0);

  const useAsBackground = async (m: MediaItem, kind: ThemeKind) => {
    setMenu(null);
    const rows = await api.listThemes();
    const row = rows.find((r) => r.kind === kind && r.is_default) ?? rows.find((r) => r.kind === kind);
    if (!row) return;
    if (!confirm(`Use “${m.title}” as the background of the theme “${row.name}”? It is used for ${kind === "song" ? "songs" : "scripture"}.`)) return;
    const t = structuredClone(row.theme);
    t.background =
      m.kind === "video"
        ? { ...t.background, kind: "video", video: `media:${m.path}`, poster: m.thumb ? (m.thumb.startsWith("data:") ? m.thumb : `media:${m.thumb}`) : null, fit: "cover" }
        : { ...t.background, kind: "image", image: `media:${m.path}`, fit: "cover" };
    try {
      await api.saveTheme(row.id, t);
      setNote({ text: `“${row.name}” now uses “${m.title}”. Change the darkening in Themes if the words are hard to read.` });
    } catch (e) {
      setNote({ text: String(e), bad: true });
    }
  };

  const live = useGoLive();
  // "My collections" for media (Easter backgrounds…)
  const [colls, setColls] = useState<ItemCollection[]>([]);
  const [showColl, setShowColl] = useState<number | null>(null);
  const [collIds, setCollIds] = useState<number[]>([]);
  const loadColls = () => api.itemCollections("media").then(setColls).catch(() => {});
  useEffect(() => void loadColls(), []);
  useEffect(() => {
    if (showColl == null) return setCollIds([]);
    api.itemCollectionItems(showColl).then((xs) => setCollIds(xs.flatMap((x) => (x.kind === "media" ? [x.media_id] : [])))).catch(() => {});
  }, [showColl, colls]);
  const toCollMany = async (ms: MediaItem[]) => {
    const name = prompt("Name of the new collection (e.g. Easter backgrounds)");
    if (!name?.trim()) return;
    const cid = await api.itemCollectionCreate("media", name);
    for (const m of ms) await api.itemCollectionAdd(cid, { kind: "media", media_id: m.id, title: m.title });
    void loadColls();
  };
  const toColl = async (m: MediaItem, id: number | null) => {
    let cid = id;
    if (cid == null) {
      const name = prompt("Name of the new collection (e.g. Easter backgrounds)");
      if (!name?.trim()) return;
      cid = await api.itemCollectionCreate("media", name);
    }
    await api.itemCollectionAdd(cid, { kind: "media", media_id: m.id, title: m.title });
    void loadColls();
  };
  // EasyWorship's media menu (right-click a picture, video or sound), then the extras
  const menuFor = async (m: MediaItem, g: MediaGridControl): Promise<MenuEntry[]> => {
    if (g.selection.length > 1) {
      const sel = g.selection;
      const n = sel.length;
      return [
        { label: `&Add ${n} items to the schedule`, run: () => sel.forEach((x) => !x.missing && onAdd(x)) },
        {
          label: `Add ${n} to co&llection`,
          sub: [...colls.map((c) => ({ label: c.name, run: () => void Promise.all(sel.map((x) => api.itemCollectionAdd(c.id, { kind: "media", media_id: x.id, title: x.title }))).then(loadColls) })), ...(colls.length ? ["-" as const] : []), { label: "New collection…", run: () => void toCollMany(sel) }],
        },
        "-",
        {
          label: `&Delete ${n} items`,
          run: async () => {
            if (!confirm(`Delete ${n} items from the media library? Themes or items using them will show nothing in their place.`)) return;
            const failed: string[] = [];
            for (const x of sel) await api.deleteMedia(x.id).catch(() => failed.push(x.title));
            g.clearSelection();
            g.refresh();
            setNote({ text: failed.length ? `Deleted ${n - failed.length}; could not delete: ${failed.join(", ")}.` : `Deleted ${n} items.`, bad: failed.length > 0 });
          },
        },
        "-",
        { label: "Select none", run: g.clearSelection },
      ];
    }
    const what = m.kind === "image" ? "Image" : m.kind === "video" ? "Video" : "Audio";
    const used = await api.mediaUsedBy(m.id).catch(() => [] as string[]);
    const logo = (await api.getSettings().catch(() => ({}) as Record<string, string>)).logo_media_id;
    const set = (repeat: MediaItem["repeat"], fit: MediaItem["fit"]) =>
      void api.setMediaOptions(m.id, repeat ?? null, fit ?? null).then(g.refresh, (e) => setNote({ text: String(e), bad: true }));
    const item = { kind: "media" as const, media_id: m.id, title: m.title };
    return [
      { label: `&Import New ${what} File...`, run: g.importFiles },
      "-",
      { label: "Media &Usage", sub: used.length ? used.map((u) => ({ label: u, disabled: true })) : [{ label: "Not used by any theme", disabled: true }] },
      ...(m.kind !== "image"
        ? [{ label: "Re&peating", sub: [{ label: "Repeat &Off", checked: m.repeat !== "loop", run: () => set("once", m.fit) }, { label: "Repeat O&n", checked: m.repeat === "loop", run: () => set("loop", m.fit) }] } as MenuEntry]
        : []),
      ...(m.kind !== "audio"
        ? [
            {
              label: "&Aspect Ratio",
              sub: [
                { label: "&Fit (whole picture)", checked: !m.fit || m.fit === "fit", run: () => set(m.repeat, "fit") },
                { label: "F&ill (crop to the screen)", checked: m.fit === "fill", run: () => set(m.repeat, "fill") },
                { label: "&Stretch", checked: m.fit === "stretch", run: () => set(m.repeat, "stretch") },
              ],
            } as MenuEntry,
          ]
        : []),
      {
        label: `&Edit ${what} Properties...`,
        run: async () => {
          const t = prompt(`Title (${m.path.split("/").pop()}${m.width ? `, ${m.width}×${m.height}` : ""}${m.duration_ms ? `, ${Math.round(m.duration_ms / 1000)} s` : ""})`, m.title);
          if (t?.trim()) await api.renameMedia(m.id, t.trim(), m.tags).then(() => (setNote({ text: "Renamed." }), g.refresh()));
        },
      },
      "-",
      ...(m.kind !== "audio"
        ? [
            {
              label: "Cop&y to Theme",
              sub: [
                { label: "&Song theme", run: () => void useAsBackground(m, "song") },
                { label: "S&cripture theme", run: () => void useAsBackground(m, "scripture") },
                { label: "&Presentation theme", run: () => void useAsBackground(m, "announcement") },
              ],
            } as MenuEntry,
            {
              label: "Use As &Logo Background",
              checked: logo === String(m.id),
              run: () =>
                void api
                  .setSetting("logo_media_id", logo === String(m.id) ? "" : String(m.id))
                  .then(() => setNote({ text: logo === String(m.id) ? "The logo screen is plain again." : `The logo screen now shows “${m.title}”.` })),
            } as MenuEntry,
            "-" as MenuEntry,
          ]
        : []),
      { label: "&Delete", run: () => void remove(m) },
      "-",
      { label: "Up&date items in Schedule", disabled: true },
      "-",
      { label: "&Open File Location", disabled: m.missing, run: () => void api.revealMedia(m.id).catch((e) => setNote({ text: String(e), bad: true })) },
      "-",
      {
        label: "&View",
        sub: [
          { label: "&List", checked: g.layout === "list", run: () => g.setLayout("list") },
          { label: "&Pictures", checked: g.layout === "thumbs", run: () => g.setLayout("thumbs") },
        ],
      },
      {
        label: "&Sort by",
        sub: [
          { label: "&Title", checked: g.sort === "title", run: () => g.setSort("title") },
          { label: "&File name", checked: g.sort === "file", run: () => g.setSort("file") },
          { label: "Date &added", checked: g.sort === "newest", run: () => g.setSort("newest") },
          { label: "&Size", checked: g.sort === "size", run: () => g.setSort("size") },
        ],
      },
      { label: "&Refresh", run: g.refresh },
      "-",
      {
        label: "Add to co&llection",
        sub: [...colls.map((c) => ({ label: c.name, run: () => void toColl(m, c.id) })), ...(colls.length ? ["-" as const] : []), { label: "New collection…", run: () => void toColl(m, null) }],
      },
      ...(showColl != null ? [{ label: "Remove from this collection", run: () => void api.itemCollectionRemove(showColl, { kind: "media", media_id: m.id, title: m.title }).then(loadColls) } as MenuEntry] : []),
      "-",
      { label: "Pre&view", run: () => onPreview(m) },
      { label: "&Go live", run: () => live(item) },
      { label: "Add to sc&hedule", run: () => onAdd(m) },
    ];
  };

  const remove = async (m: MediaItem) => {
    setMenu(null);
    const used = await api.mediaUsedBy(m.id);
    const warn = used.length ? `\n\nThese themes use it and will lose their background: ${used.join(", ")}.` : "";
    if (!confirm(`Delete “${m.title}” from this profile?${warn}`)) return;
    await api.deleteMedia(m.id);
    setNote({ text: `“${m.title}” deleted.` });
  };

  return (
    <>
      <div className="media__where" role="radiogroup" aria-label="Media from">
        <button role="radio" aria-checked={!online} className={!online ? "is-on" : ""} onClick={() => setOnline(false)}>
          Library
        </button>
        <button role="radio" aria-checked={online} className={online ? "is-on" : ""} onClick={() => setOnline(true)} title="Free pictures and videos from Pixabay and Unsplash">
          Online
        </button>
      </div>
      {online && <OnlineMedia onDownloaded={() => setBump((x) => x + 1)} />}
      {note && <p className={note.bad ? "form-error" : "notice"}>{note.text}</p>}
      {colls.length > 0 && (
        <label className="media__show">
          Show{" "}
          <select value={showColl ?? ""} onChange={(e) => setShowColl(e.target.value ? Number(e.target.value) : null)} aria-label="Show media">
            <option value="">All media</option>
            {colls.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name} ({c.count})
              </option>
            ))}
          </select>
        </label>
      )}
      {!online && (
      <MediaGrid
        key={version + (note?.text ?? "") + bump}
        onPick={onPreview}
        draggableItems
        menuFor={menuFor}
        onlyIds={showColl == null ? null : collIds}
        actions={(m) => (
          <>
            <button className="icon-btn" aria-label={`Add ${m.title} to the service`} title="Add to the service" onClick={() => onAdd(m)}>
              <Icon name="plus" />
            </button>
            <button className="icon-btn" aria-label={`More for ${m.title}`} title="More" onClick={() => setMenu(menu === m.id ? null : m.id)}>
              ⋯
            </button>
            {menu === m.id && (
              <div className="menu-pop" role="menu">
                {m.kind !== "audio" && (
                  <>
                    <button role="menuitem" onClick={() => useAsBackground(m, "song")}>
                      Background for songs
                    </button>
                    <button role="menuitem" onClick={() => useAsBackground(m, "scripture")}>
                      Background for scripture
                    </button>
                  </>
                )}
                <button
                  role="menuitem"
                  onClick={async () => {
                    setMenu(null);
                    const t = prompt("Name", m.title);
                    if (t?.trim()) await api.renameMedia(m.id, t.trim(), m.tags).then(() => setNote({ text: "Renamed." }));
                  }}
                >
                  Rename
                </button>
                <button role="menuitem" className="btn-danger" onClick={() => remove(m)}>
                  Delete
                </button>
              </div>
            )}
          </>
        )}
      />
      )}
        </>
  );
}
