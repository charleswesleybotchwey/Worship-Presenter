import { useState } from "react";
import { api } from "./api";

// Finding a song's words online: Hymnary.org (hymns, public-domain texts) and LRCLIB
// (an open lyrics database). The words become a new song, opened in the editor to check.

type Found = { source: string; key: string; title: string; author: string; words: string | null };

export function LyricsFinder({ onClose, onAdded }: { onClose: () => void; onAdded: (songId: number, title: string) => void }) {
  const [title, setTitle] = useState("");
  const [artist, setArtist] = useState("");
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<Found[] | null>(null);
  const [pick, setPick] = useState<{ f: Found; words: string } | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const run = async <T,>(f: () => Promise<T>) => {
    setBusy(true);
    setProblem(null);
    try {
      return await f();
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  };
  const search = () => run(async () => (setPick(null), setResults(await api.lyricsSearch(title, artist))));
  const open = (f: Found) => run(async () => setPick({ f, words: f.words ?? (await api.lyricsWords(f.source, f.key)) }));
  const add = () => pick && run(async () => onAdded(await api.lyricsAdd(pick.f.title, pick.f.author, pick.words, pick.f.source === "Hymnary" ? "Hymnary.org" : "LRCLIB"), pick.f.title));
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog wp-dialog--wide lyf" role="dialog" aria-modal="true" aria-label="Find lyrics online">
        <header className="wp-dialog__head">
          <h2>Find lyrics online</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        <div className="lyf__body">
          <form className="lyf__search" onSubmit={(e) => (e.preventDefault(), void search())}>
            <input className="wp-input" placeholder="Song title" value={title} onChange={(e) => setTitle(e.target.value)} aria-label="Song title" autoFocus />
            <input className="wp-input" placeholder="Artist or author (optional)" value={artist} onChange={(e) => setArtist(e.target.value)} aria-label="Artist" />
            <button className="wp-btn wp-btn--primary" disabled={busy || !title.trim()}>
              {busy && !pick ? "Searching…" : "Search"}
            </button>
          </form>
          <p className="wp-note">From Hymnary.org (hymns; full words of public-domain texts) and LRCLIB (an open lyrics database). Projecting copyrighted songs needs your church's CCLI licence.</p>
          {problem && <p className="form-error">{problem}</p>}
          <div className="lyf__cols">
            <div className="lyf__list">
              {results?.length === 0 && <p className="wp-note">Nothing found. Try fewer words, or without the artist.</p>}
              {results?.map((f) => (
                <button key={f.source + f.key} className={`lyf__row ${pick?.f === f ? "is-current" : ""}`} onClick={() => void open(f)}>
                  <span className={`lyf__src lyf__src--${f.source.toLowerCase()}`}>{f.source}</span>
                  <span>
                    <strong>{f.title}</strong>
                    <small>{f.author}</small>
                  </span>
                </button>
              ))}
            </div>
            <div className="lyf__preview">
              {pick ? (
                <>
                  <pre>{pick.words}</pre>
                  <button className="wp-btn wp-btn--primary" disabled={busy} onClick={() => void add()}>
                    Add to library and check it
                  </button>
                </>
              ) : (
                <p className="wp-note">{results ? "Choose a result to see its words." : "Search by title, and artist if you know it."}</p>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
