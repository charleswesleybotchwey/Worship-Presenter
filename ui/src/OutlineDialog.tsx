import { useEffect, useState } from "react";
import { api, type Deck } from "./api";

// The pastor's outline pasted in: each line a point slide, every scripture reference
// followed by its passage. Checked here, then saved as a presentation.

export function OutlineDialog({ onClose, onMade }: { onClose: () => void; onMade: (title: string) => void }) {
  const [text, setText] = useState(() => localStorage.getItem("wp-outline") || "");
  const [bibles, setBibles] = useState<{ code: string; name: string }[]>([]);
  const [bible, setBible] = useState(() => localStorage.getItem("wp-outline-bible") || "");
  const [deck, setDeck] = useState<Deck | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    api.versions().then((v) => (setBibles(v), setBible((b) => b || v[0]?.code || ""))).catch(() => {});
  }, []);
  const make = async () => {
    try {
      localStorage.setItem("wp-outline", text);
      localStorage.setItem("wp-outline-bible", bible);
      setDeck(await api.outlineDeck(text, bible));
      setProblem(null);
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };
  const save = async () => {
    if (!deck) return;
    try {
      await api.saveAnnouncement(deck);
      onMade(deck.title);
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog wp-dialog--wide outl" role="dialog" aria-modal="true" aria-label="Sermon outline to slides">
        <header className="wp-dialog__head">
          <h2>Sermon outline to slides</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        <div className="outl__body">
          <div className="outl__in">
            <p className="wp-note">Paste the outline: the first line is the title, each other line a point. Scripture references in it (Romans 8:28, 1 Cor. 13:4-7, Ps 23) get their passage on slides.</p>
            <textarea className="wp-input outl__text" value={text} onChange={(e) => (setText(e.target.value), setDeck(null))} placeholder={"The God Who Works for Our Good\n1. He works in all things (Romans 8:28)\n2. His love never fails - 1 Cor 13:4-7"} aria-label="Outline" />
            <label className="ps__row">
              <span>Readings from</span>
              <select className="wp-input" value={bible} onChange={(e) => (setBible(e.target.value), setDeck(null))} aria-label="Bible">
                {bibles.map((b) => (
                  <option key={b.code} value={b.code}>
                    {b.code} {b.name !== b.code ? `· ${b.name}` : ""}
                  </option>
                ))}
              </select>
            </label>
            <button className="wp-btn" disabled={!text.trim() || !bible} onClick={make}>
              Make the slides
            </button>
          </div>
          <div className="outl__out">
            {deck ? (
              <>
                <p className="wp-note">{deck.slides.length} slides. Check them, then save; you can edit them afterwards in Presentations.</p>
                <ol className="outl__list">
                  {deck.slides.map((s, i) => (
                    <li key={i} className={s.title && i > 0 ? "is-reading" : ""}>
                      {s.title && <strong>{s.title}</strong>} {s.body}
                    </li>
                  ))}
                </ol>
                <button className="wp-btn wp-btn--primary" onClick={save}>
                  Save as a presentation
                </button>
              </>
            ) : (
              <p className="wp-note">The slides appear here.</p>
            )}
            {problem && <p className="form-error">{problem}</p>}
          </div>
        </div>
      </div>
    </div>
  );
}
