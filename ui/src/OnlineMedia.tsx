import { useEffect, useState } from "react";
import { api, type MediaItem } from "./api";

// Free pictures and videos from Pixabay and Unsplash, downloaded into the media library
// with the photographer's credit. Each service needs the church's own free key.

type Found = { source: string; id: string; kind: string; title: string; thumb: string; file: string; width: number; height: number; author: string; author_url: string; page_url: string; track: string | null };
const SOURCES = [
  { id: "pixabay-photo", label: "Pixabay pictures", service: "pixabay" },
  { id: "pixabay-video", label: "Pixabay videos", service: "pixabay" },
  { id: "unsplash", label: "Unsplash pictures", service: "unsplash" },
];
const HOW: Record<string, string> = {
  pixabay: "Free key: sign up at pixabay.com, then open pixabay.com/api/docs (your key is shown there).",
  unsplash: "Free key: unsplash.com/developers → New Application → copy the Access Key.",
};

export function OnlineMedia({ onDownloaded }: { onDownloaded: (m: MediaItem) => void }) {
  const [source, setSource] = useState(() => localStorage.getItem("wp-online-source") || "pixabay-photo");
  const [q, setQ] = useState("");
  const [page, setPage] = useState(1);
  const [results, setResults] = useState<Found[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  const [keys, setKeys] = useState<Record<string, boolean>>({});
  const [typing, setTyping] = useState("");
  const service = SOURCES.find((s) => s.id === source)!.service;
  const loadKeys = () => api.onlineKeysSaved().then(setKeys).catch(() => {});
  useEffect(() => void loadKeys(), []);
  useEffect(() => localStorage.setItem("wp-online-source", source), [source]);
  const search = async (p = 1) => {
    if (!q.trim()) return;
    setBusy("search");
    setMsg(null);
    try {
      const r = await api.onlineSearch(source, q, p);
      setPage(p);
      setResults(p === 1 ? r : [...(results ?? []), ...r]);
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    } finally {
      setBusy(null);
    }
  };
  const download = async (f: Found) => {
    setBusy(f.id);
    try {
      const m = await api.onlineDownload(f);
      setMsg({ text: `“${m.title}” is in the media library.` });
      onDownloaded(m);
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    } finally {
      setBusy(null);
    }
  };
  if (!keys[service])
    return (
      <div className="onl onl--key">
        <select className="wp-input" value={source} onChange={(e) => setSource(e.target.value)} aria-label="Source">
          {SOURCES.map((s) => (
            <option key={s.id} value={s.id}>
              {s.label}
            </option>
          ))}
        </select>
        <p className="wp-note">{HOW[service]}</p>
        <div className="onl__row">
          <input className="wp-input" type="password" autoComplete="off" placeholder={`Paste your ${service === "unsplash" ? "Unsplash Access" : "Pixabay"} key`} value={typing} onChange={(e) => setTyping(e.target.value)} aria-label="Key" />
          <button className="wp-btn wp-btn--primary" disabled={!typing.trim()} onClick={() => void api.onlineKeySet(service, typing).then(() => (setTyping(""), loadKeys()))}>
            Save key
          </button>
        </div>
        <p className="wp-note">Kept protected on this computer.</p>
      </div>
    );
  return (
    <div className="onl">
      <form className="onl__row" onSubmit={(e) => (e.preventDefault(), void search(1))}>
        <select className="wp-input" value={source} onChange={(e) => (setSource(e.target.value), setResults(null))} aria-label="Source">
          {SOURCES.map((s) => (
            <option key={s.id} value={s.id}>
              {s.label}
            </option>
          ))}
        </select>
        <input className="wp-input onl__q" placeholder="Search: cross, sunrise, worship, nature…" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search online" />
        <button className="wp-btn" disabled={!!busy || !q.trim()}>
          {busy === "search" ? "Searching…" : "Search"}
        </button>
      </form>
      {msg && <p className={msg.bad ? "form-error" : "wp-note"}>{msg.text}</p>}
      <div className="onl__grid">
        {results?.length === 0 && <p className="wp-note">Nothing found. Try another word.</p>}
        {results?.map((f) => (
          <figure key={f.source + f.id} className="onl__item">
            <img src={f.thumb} alt={f.title} loading="lazy" />
            <figcaption>
              <span className="onl__title">{f.title}</span>
              <button className="btn-quiet onl__credit" onClick={() => void api.onlineOpen(f.author_url || f.page_url).catch(() => {})} title="Open the photographer's page">
                {f.kind === "video" ? "Video" : "Photo"} by {f.author || "unknown"} on {f.source === "unsplash" ? "Unsplash" : "Pixabay"}
              </button>
              <button className="wp-btn wp-btn--quiet" disabled={!!busy} onClick={() => void download(f)}>
                {busy === f.id ? "Downloading…" : "Download"}
              </button>
            </figcaption>
          </figure>
        ))}
      </div>
      {results && results.length >= 30 * page && (
        <button className="wp-btn wp-btn--quiet" disabled={!!busy} onClick={() => void search(page + 1)}>
          More
        </button>
      )}
      <button className="btn-quiet onl__forget" onClick={() => confirm(`Remove the saved ${service} key from this computer?`) && void api.onlineKeySet(service, "").then(loadKeys)}>
        Remove the {service === "unsplash" ? "Unsplash" : "Pixabay"} key
      </button>
    </div>
  );
}
