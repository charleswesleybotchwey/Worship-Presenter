import { useCallback, useEffect, useState } from "react";
import { api, pickMediaFiles, type MediaItem } from "./api";
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

export function MediaGrid({ kind, onPick, selectedId, actions }: { kind?: MediaItem["kind"] | ""; onPick: (m: MediaItem) => void; selectedId?: number | null; actions?: (m: MediaItem) => React.ReactNode }) {
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
          {items.map((m) => (
            <li
              key={m.id}
              role="option"
              aria-selected={m.id === selectedId}
              className={`wp-row wp-row--lib ${m.id === selectedId ? "is-selected" : ""}`}
              title={m.title}
              onClick={() => (m.missing ? setProblem(`“${m.title}” is missing from the media folder. Use Relink to point it at the file.`) : onPick(m))}
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
        {layout === "thumbs" && items.map((m) => {
          const t = thumbSrc(m, url);
          return (
            <li key={m.id} className={m.id === selectedId ? "is-current" : ""}>
              <button className="media__card" onClick={() => onPick(m)} title={m.title}>
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
    </div>
  );
}

export function MediaLibrary({ onPreview, onAdd, version }: { onPreview: (m: MediaItem) => void; onAdd: (m: MediaItem) => void; version: number }) {
  const [menu, setMenu] = useState<number | null>(null);
  const [note, setNote] = useState<{ text: string; bad?: boolean } | null>(null);

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
      {note && <p className={note.bad ? "form-error" : "notice"}>{note.text}</p>}
      <MediaGrid
        key={version + (note?.text ?? "")}
        onPick={onPreview}
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
    </>
  );
}
