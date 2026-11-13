import { useEffect, useMemo, useState } from "react";
import { api } from "./api";

// "More available…": eBible.org's freely shareable Bibles, downloaded into the library.

export function BibleStore({ onClose, onAdded }: { onClose: () => void; onAdded: (title: string) => void }) {
  const [list, setList] = useState<{ id: string; language: string; title: string }[] | null>(null);
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [progress, setProgress] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [codes, setCodes] = useState<Record<string, string>>({});
  useEffect(() => {
    api.bibleCatalogue().then(setList, (e) => setProblem(String(e instanceof Error ? e.message : e)));
  }, []);
  useEffect(() => {
    if (!busy) return;
    const t = setInterval(() => api.speechStatus().then((s) => setProgress(s.installing)).catch(() => {}), 500);
    return () => clearInterval(t);
  }, [busy]);
  const shown = useMemo(() => {
    const f = q.trim().toLowerCase();
    return (list ?? []).filter((b) => !f || b.language.toLowerCase().includes(f) || b.title.toLowerCase().includes(f) || b.id.toLowerCase().includes(f)).slice(0, 200);
  }, [list, q]);
  const codeFor = (id: string) => codes[id] ?? (id.replace(/^[a-z]{3}(?=[a-z0-9])/, "").toUpperCase().slice(0, 8) || id.toUpperCase());
  const get = async (b: { id: string; language: string; title: string }) => {
    setBusy(b.id);
    setProblem(null);
    try {
      await api.bibleDownload(b.id, codeFor(b.id), b.title, b.language);
      onAdded(b.title);
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(null);
      setProgress(null);
    }
  };
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && !busy && onClose()}>
      <div className="wp-dialog wp-dialog--wide bs" role="dialog" aria-modal="true" aria-label="More Bibles">
        <header className="wp-dialog__head">
          <h2>More Bibles</h2>
          <span className="wp-note wp-note--inline">Free to share, from eBible.org. Needs internet once; then they work offline like your other Bibles.</span>
          <button className="wp-btn wp-btn--quiet" disabled={!!busy} onClick={onClose}>
            Close
          </button>
        </header>
        <div className="bs__body">
          <input className="wp-input" placeholder="Language or title: Twi, Ewe, Akan, King James…" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find a Bible" autoFocus />
          {!list && !problem && <p className="wp-note">Getting the list from eBible.org…</p>}
          {progress && <p className="wp-note">{progress}</p>}
          {problem && <p className="form-error">{problem}</p>}
          <div className="bs__list">
            {shown.map((b) => (
              <div key={b.id} className="bs__row">
                <span className="bs__lang">{b.language}</span>
                <span className="bs__title">{b.title}</span>
                <input className="wp-input bs__code" value={codeFor(b.id)} maxLength={8} onChange={(e) => setCodes({ ...codes, [b.id]: e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, "") })} aria-label={`Short name for ${b.title}`} title="The short name shown in the Bible list" />
                <button className="wp-btn" disabled={!!busy} onClick={() => void get(b)}>
                  {busy === b.id ? "Downloading…" : "Download"}
                </button>
              </div>
            ))}
            {list && shown.length === 0 && <p className="wp-note">Nothing matches “{q}”.</p>}
          </div>
        </div>
      </div>
    </div>
  );
}
