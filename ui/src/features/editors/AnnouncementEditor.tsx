import { useEffect, useRef, useState } from "react";
import { api, type Deck, type DeckSlide, type Slide } from "../../api";
import { SlideView } from "../../SlideView";
import { MediaGrid } from "../../MediaLibrary";
import { Icon } from "../../Icon";

// Announcements editor (design brief §5): slide list left, true-aspect canvas centre,
// properties right. Saving never changes what is on the screens.

const CHARS = ["ɛ", "ɔ", "Ɛ", "Ɔ", "ŋ", "Ŋ"];

const toSlide = (s: DeckSlide, i: number): Slide => ({
  label: s.title.trim() || `Slide ${i + 1}`,
  occurrence: i,
  first_key: i,
  last_key: i,
  primary: s.kind === "text" ? s.body.split("\n").filter((l) => l.trim()) : [],
  second: [],
  image: s.kind === "picture" ? s.image : null,
});

export function AnnouncementEditor({ deckId, template, onClose, onSaved }: { deckId: number | null; template: Deck | null; onClose: () => void; onSaved: (id: number, title: string) => void }) {
  const [deck, setDeck] = useState<Deck | null>(template ? structuredClone(template) : deckId == null ? { id: null, title: "New announcements", looping: false, seconds: 0, slides: [{ kind: "text", title: "", body: "", image: null, seconds: null }] } : null);
  const [sel, setSel] = useState(0);
  const [dirty, setDirty] = useState(!!template);
  const [problem, setProblem] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  const body = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (deckId != null && !template) api.getAnnouncement(deckId).then(setDeck).catch((e) => setProblem(String(e)));
  }, [deckId, template]);

  if (!deck) return <div className="wp-page">{problem ? <p className="wp-banner wp-banner--bad">{problem}</p> : <p className="wp-note">Opening…</p>}</div>;

  const change = (d: Deck) => (setDeck(d), setDirty(true), setProblem(null));
  const slide = deck.slides[sel];
  const setSlide = (p: Partial<DeckSlide>) => change({ ...deck, slides: deck.slides.map((s, i) => (i === sel ? { ...s, ...p } : s)) });
  const add = (kind: DeckSlide["kind"]) => {
    const slides = [...deck.slides];
    slides.splice(sel + 1, 0, { kind, title: "", body: "", image: null, seconds: null });
    change({ ...deck, slides });
    setSel(sel + 1);
    if (kind === "picture") setPicking(true);
  };
  const move = (d: number) => {
    const j = sel + d;
    if (j < 0 || j >= deck.slides.length) return;
    const slides = [...deck.slides];
    [slides[sel], slides[j]] = [slides[j], slides[sel]];
    change({ ...deck, slides });
    setSel(j);
  };
  const insertChar = (c: string) => {
    const el = body.current;
    if (!el || !slide) return;
    const s = el.selectionStart, e = el.selectionEnd;
    setSlide({ body: slide.body.slice(0, s) + c + slide.body.slice(e) });
    requestAnimationFrame(() => (el.focus(), el.setSelectionRange(s + c.length, s + c.length)));
  };
  const save = async () => {
    try {
      const id = await api.saveAnnouncement(deck);
      setDirty(false);
      onSaved(id, deck.title);
      onClose();
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };
  const close = () => (!dirty || confirm("Close without saving your changes?")) && onClose();

  return (
    <div
      className="wp-page"
      onKeyDown={(e) => {
        if (e.ctrlKey && e.key.toLowerCase() === "s") e.preventDefault(), void save();
      }}
    >
      <header className="wp-editor-head">
        <h1 className="wp-editor-title">{deck.title || "Announcements"}</h1>
        {dirty && <span className="wp-note wp-note--inline">Unsaved changes</span>}
      </header>
      <div className="ann">
        <aside className="ann__list">
          <ol className="wp-rows" role="listbox" aria-label="Slides">
            {deck.slides.map((s, i) => (
              <li key={i} role="option" aria-selected={i === sel} tabIndex={i === sel ? 0 : -1} className={`wp-row ${i === sel ? "is-selected" : ""}`} onClick={() => setSel(i)}>
                <span className="wp-row__num">{String(i + 1).padStart(2, "0")}</span>
                <Icon name={s.kind === "picture" ? "image" : "slides"} />
                <span className="wp-row__title">{s.title.trim() || (s.kind === "picture" ? "Picture" : s.body.split("\n")[0] || "Empty slide")}</span>
              </li>
            ))}
          </ol>
          <div className="wp-actions">
            <button className="wp-btn wp-btn--quiet" onClick={() => add("text")}>
              + Text
            </button>
            <button className="wp-btn wp-btn--quiet" onClick={() => add("picture")}>
              + Picture
            </button>
          </div>
          <div className="wp-actions wp-actions--bare">
            <button className="wp-icon-btn" title="Move up" aria-label="Move slide up" onClick={() => move(-1)} disabled={sel === 0}>
              ↑
            </button>
            <button className="wp-icon-btn" title="Move down" aria-label="Move slide down" onClick={() => move(1)} disabled={sel === deck.slides.length - 1}>
              ↓
            </button>
            <button className="wp-btn wp-btn--quiet" onClick={() => slide && (change({ ...deck, slides: [...deck.slides.slice(0, sel + 1), structuredClone(slide), ...deck.slides.slice(sel + 1)] }), setSel(sel + 1))}>
              Duplicate
            </button>
            <button
              className="wp-btn wp-btn--quiet btn-danger"
              disabled={deck.slides.length < 2}
              onClick={() => (change({ ...deck, slides: deck.slides.filter((_, i) => i !== sel) }), setSel(Math.max(0, sel - 1)))}
            >
              Remove
            </button>
          </div>
        </aside>

        <main className="ann__canvas">
          <div className="ann__frame">{slide && <SlideView slide={toSlide(slide, sel)} kind="announcement" />}</div>
          <p className="wp-note">
            Slide {sel + 1} of {deck.slides.length}
            {deck.seconds > 0 || slide?.seconds ? ` · shows for ${slide?.seconds ?? deck.seconds} s when advancing by itself` : " · moves on when you press Next"}
          </p>
        </main>

        <aside className="ann__props">
          {slide && (
            <section className="prop-group">
              <h3>Slide</h3>
              <label className="prop">
                <span>Type</span>
                <select value={slide.kind} onChange={(e) => setSlide({ kind: e.target.value as DeckSlide["kind"] })}>
                  <option value="text">Title and text</option>
                  <option value="picture">Picture</option>
                </select>
              </label>
              <label className="prop">
                <span>Title</span>
                <input value={slide.title} onChange={(e) => setSlide({ title: e.target.value })} placeholder={slide.kind === "picture" ? "Name in the list (not shown)" : "Offering"} />
              </label>
              {slide.kind === "text" ? (
                <>
                  <label className="prop prop--block">
                    <span>Text</span>
                    <textarea ref={body} rows={6} value={slide.body} onChange={(e) => setSlide({ body: e.target.value })} />
                  </label>
                  <div className="prop-buttons">
                    {CHARS.map((c) => (
                      <button key={c} className="char-btn" onClick={() => insertChar(c)} title={`Type ${c}`}>
                        {c}
                      </button>
                    ))}
                  </div>
                </>
              ) : (
                <div className="prop-buttons">
                  <button className="wp-btn" onClick={() => setPicking(true)}>
                    {slide.image ? "Change picture…" : "Choose picture…"}
                  </button>
                </div>
              )}
              <label className="prop">
                <span>Shows for</span>
                <select value={slide.seconds ?? ""} onChange={(e) => setSlide({ seconds: e.target.value ? Number(e.target.value) : null })}>
                  <option value="">Same as the others</option>
                  {[5, 8, 10, 12, 15, 20, 30, 45, 60].map((n) => (
                    <option key={n} value={n}>
                      {n} seconds
                    </option>
                  ))}
                </select>
              </label>
            </section>
          )}
          <section className="prop-group">
            <h3>These announcements</h3>
            <label className="prop">
              <span>Name</span>
              <input value={deck.title} onChange={(e) => change({ ...deck, title: e.target.value })} />
            </label>
            <label className="prop">
              <span>Moves on</span>
              <select value={deck.seconds} onChange={(e) => change({ ...deck, seconds: Number(e.target.value) })}>
                <option value={0}>When I press Next</option>
                {[5, 8, 10, 12, 15, 20, 30].map((n) => (
                  <option key={n} value={n}>
                    By itself every {n} s
                  </option>
                ))}
              </select>
            </label>
            <label className="prop prop--check">
              <input type="checkbox" checked={deck.looping} onChange={(e) => change({ ...deck, looping: e.target.checked })} />
              <span>Start again after the last slide (loop before the service)</span>
            </label>
          </section>
          {problem && <p className="wp-banner wp-banner--bad">{problem}</p>}
        </aside>
      </div>
      <footer className="dialog__foot">
        {deck.id != null && (
          <button
            className="btn-quiet btn-danger"
            onClick={async () => {
              if (!confirm(`Delete “${deck.title}”?`)) return;
              try {
                await api.deleteAnnouncement(deck.id!);
                onSaved(-1, "");
                onClose();
              } catch (e) {
                setProblem(String(e instanceof Error ? e.message : e));
              }
            }}
          >
            Delete
          </button>
        )}
        <span className="wp-spacer" />
        <button className="btn-quiet" onClick={close}>
          Cancel
        </button>
        <button className="btn" onClick={save} title="Save for every service. Never changes what is on the screens.">
          Save to library <kbd>Ctrl+S</kbd>
        </button>
      </footer>
      {picking && (
        <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && setPicking(false)}>
          <div className="wp-dialog dialog--picker" role="dialog" aria-label="Choose a picture">
            <header className="wp-dialog__head">
              <h2>Choose a picture</h2>
              <button className="wp-btn wp-btn--quiet" onClick={() => setPicking(false)}>
                Cancel
              </button>
            </header>
            <MediaGrid kind="image" onPick={(m) => (setSlide({ image: m.path, title: slide?.title || m.title }), setPicking(false))} />
          </div>
        </div>
      )}
    </div>
  );
}
