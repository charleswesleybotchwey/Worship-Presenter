import { useEffect, useMemo, useRef, useState } from "react";
import { ArrangementEditor } from "./features/editors/ArrangementEditor";
import { sectionsOf } from "./groups";
import { capitaliseLines, dropBlankLines, tidySpaces, trimLineEnds } from "./songTidy";
import { api, type Slide, type SongDoc } from "./api";
import { SlideView } from "./SlideView";

// Edit or create a song. Lyrics are plain text per language with [V1] / [C] labels;
// the right side shows the slides exactly as they will project, updated as you type.

export const LANGS: { code: string; name: string }[] = [
  { code: "en", name: "English" },
  { code: "fat", name: "Fante" },
  { code: "tw", name: "Twi" },
  { code: "gaa", name: "Ga" },
  { code: "ee", name: "Ewe" },
  { code: "dag", name: "Dagbani" },
];
const langName = (c: string) => LANGS.find((l) => l.code === c)?.name ?? c;

// Most keyboards here cannot type these directly.
const CHARS = ["ɛ", "ɔ", "Ɛ", "Ɔ", "ŋ", "Ŋ", "ɣ", "ʒ"];

const EMPTY: SongDoc = { id: null, title: "", author: null, copyright: null, ccli: null, numbers: [], lyrics: [{ lang: "en", body: "[V1]\n" }], sequence: "", lines_per_slide: null, key: null };

function numbersToText(n: SongDoc["numbers"]) {
  return n.map((x) => `${x.hymnal} ${x.number}`).join(", ");
}
function textToNumbers(t: string): SongDoc["numbers"] {
  return t
    .split(",")
    .map((x) => x.trim().split(/\s+/))
    .filter((p) => p.length >= 2)
    .map((p) => ({ hymnal: p.slice(0, -1).join(" ").toUpperCase(), number: p[p.length - 1] }));
}

/** Lines of one language per section, for the pairing view. */
function sections(body: string) {
  const out: { label: string; lines: string[] }[] = [];
  for (const raw of body.split("\n")) {
    const t = raw.trim();
    if (!t) continue;
    const m = t.match(/^\[(.+)\]$/);
    if (m) out.push({ label: m[1].trim(), lines: [] });
    else (out[out.length - 1] ?? out[out.push({ label: "V1", lines: [] }) - 1]).lines.push(t);
  }
  return out.filter((x) => x.lines.length && x.label.toUpperCase() !== "TITLE");
}

const TIDY: Record<string, (b: string) => string> = { caps: capitaliseLines, ends: trimLineEnds, spaces: tidySpaces, blank: dropBlankLines };

export function SongEditor({
  songId,
  onClose,
  onSaved,
  onDeleted,
  serviceItem,
  onApply,
}: {
  songId: number | null;
  onClose: () => void;
  onSaved: (id: number, title: string, live: boolean) => void;
  onDeleted: (id: number) => void;
  /** Present when this song is in the open service: "Apply to this service" changes only that item. */
  serviceItem?: { lyrics?: { lang: string; body: string }[] | null } | null;
  onApply?: (lyrics: { lang: string; body: string }[] | null) => void;
}) {
  const [view, setView] = useState<"slides" | "pairing">("slides");
  const [doc, setDoc] = useState<SongDoc | null>(songId == null ? EMPTY : null);
  const [numbers, setNumbers] = useState("");
  const [tab, setTab] = useState(0);
  const [slides, setSlides] = useState<Slide[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const area = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (songId == null) return;
    api
      .getSong(songId)
      .then((d) => {
        // a song already changed for this service opens with those words
        if (serviceItem?.lyrics?.length) d = { ...d, lyrics: serviceItem.lyrics };
        setDoc(d);
        setNumbers(numbersToText(d.numbers));
      })
      .catch((e) => setProblem(String(e)));
  }, [songId]);

  const full = useMemo(() => (doc ? { ...doc, numbers: textToNumbers(numbers) } : null), [doc, numbers]);

  // live preview, debounced
  useEffect(() => {
    if (!full) return;
    const t = setTimeout(() => {
      api
        .previewSongDoc(full)
        .then((s) => (setSlides(s), setProblem(null)))
        .catch((e) => (setSlides([]), setProblem(String(e instanceof Error ? e.message : e))));
    }, 250);
    return () => clearTimeout(t);
  }, [full]);

  if (!doc || !full) {
    return (
      <div className="wp-page">
        <p className="empty">{problem ?? "Opening the song…"}</p>
      </div>
    );
  }

  const update = (p: Partial<SongDoc>) => (setDoc({ ...doc, ...p }), setDirty(true));
  const setBody = (body: string) => update({ lyrics: doc.lyrics.map((l, i) => (i === tab ? { ...l, body } : l)) });

  // rich text: wrap the selected words in markers (**bold**, *italic*, __underline__, {gold}…{/})
  const wrap = (before: string, after: string) => {
    const el = area.current;
    const body = doc.lyrics[tab].body;
    const s = el?.selectionStart ?? body.length;
    const e = el?.selectionEnd ?? body.length;
    if (s === e) return;
    setBody(body.slice(0, s) + before + body.slice(s, e) + after + body.slice(e));
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(s + before.length, e + before.length);
    });
  };
  const insert = (text: string) => {
    const el = area.current;
    const body = doc.lyrics[tab].body;
    const s = el?.selectionStart ?? body.length;
    const e = el?.selectionEnd ?? body.length;
    setBody(body.slice(0, s) + text + body.slice(e));
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(s + text.length, s + text.length);
    });
  };
  const insertSection = (kind: "V" | "C" | "B") => {
    const body = doc.lyrics[tab].body;
    const label = kind === "V" ? `V${(body.match(/^\[V\d*\]/gm)?.length ?? 0) + 1}` : kind;
    const lead = body.length && !body.endsWith("\n\n") ? (body.endsWith("\n") ? "\n" : "\n\n") : "";
    insert(`${lead}[${label}]\n`);
  };

  const save = async () => {
    setSaving(true);
    try {
      const r = await api.saveSong(full);
      setDirty(false);
      onSaved(r.id, full.title.trim(), r.live);
      onClose();
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    } finally {
      setSaving(false);
    }
  };

  const close = () => {
    if (dirty && !confirm("Close without saving your changes?")) return;
    onClose();
  };

  const remove = async () => {
    if (doc.id == null || !confirm(`Delete “${doc.title}” from the library? Service orders that use it will skip it.`)) return;
    await api.deleteSong(doc.id);
    onDeleted(doc.id);
    onClose();
  };

  const unused = LANGS.filter((l) => !doc.lyrics.some((x) => x.lang === l.code));

  // pairing view: the same even sharing-out the slides use
  const pairs = (() => {
    if (doc.lyrics.length < 2) return { rows: [], unpaired: 0 };
    const A = sections(doc.lyrics[0].body);
    const B = sections(doc.lyrics[1].body);
    let unpaired = 0;
    const rows = A.map((sa) => {
      const sb = B.find((x) => x.label.toUpperCase() === sa.label.toUpperCase());
      const a = sa.lines.length;
      const b = sb?.lines.length ?? 0;
      const n = Math.max(a, b);
      const lines: [string, string][] = Array.from({ length: n }, (_, i) => [
        sa.lines.filter((_, j) => Math.floor((j * n) / a) === i).join(" / "),
        (sb?.lines ?? []).filter((_, j) => Math.floor((j * n) / Math.max(1, b)) === i).join(" / "),
      ]);
      const uneven = !!sb && a !== b;
      if (uneven || !sb) unpaired++;
      return { label: sa.label, a, b, uneven, missing: !sb, lines };
    });
    return { rows, unpaired };
  })();

  /** Opens the language tab and selects that part's text, so the editor can fix it. */
  const fixIn = (langIdx: number, label: string) => {
    setTab(langIdx);
    requestAnimationFrame(() => {
      const el = area.current;
      if (!el) return;
      const body = doc.lyrics[langIdx].body;
      const start = body.search(new RegExp(`^\\[${label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\]`, "mi"));
      if (start < 0) return;
      const next = body.slice(start + 1).search(/^\[/m);
      el.focus();
      el.setSelectionRange(start, next < 0 ? body.length : start + 1 + next);
    });
  };

  return (
    <div className="wp-page">
      <div
        className="editor"
        role="region"
        aria-label={doc.id == null ? "New song" : `Edit ${doc.title}`}
        onKeyDown={(e) => {
          if (e.ctrlKey && e.key.toLowerCase() === "s") e.preventDefault(), void save();
          if (e.key === "Escape") e.stopPropagation(), close();
        }}
      >
        <div className="dialog__head">
          <h1 className="wp-editor-title">{doc.id == null ? "New song" : doc.title || "Edit song"}</h1>
        </div>
        <div className="editor__body">
          <div className="editor__form">
            <div className="fields fields--meta">
              <label className="field field--wide field--title">
                <span>Title</span>
                <input value={doc.title} spellCheck lang="en" onChange={(e) => update({ title: e.target.value })} autoFocus={doc.id == null} />
              </label>
              <label className="field">
                <span>Hymn numbers</span>
                <input value={numbers} placeholder="MHB 1, PAN 12" onChange={(e) => (setNumbers(e.target.value), setDirty(true))} />
              </label>
              <label className="field">
                <span>Author</span>
                <input value={doc.author ?? ""} onChange={(e) => update({ author: e.target.value })} />
              </label>
              <label className="field">
                <span>Copyright</span>
                <input value={doc.copyright ?? ""} onChange={(e) => update({ copyright: e.target.value })} />
              </label>
              <label className="field">
                <span>CCLI number</span>
                <input value={doc.ccli ?? ""} onChange={(e) => update({ ccli: e.target.value })} />
              </label>
              <label className="field" title="For the musicians' stage view. Write chords in the lyrics in brackets before the syllable: [G]Amazing [C]grace">
                <span>Key</span>
                <input value={doc.key ?? ""} placeholder="e.g. G" maxLength={6} onChange={(e) => update({ key: e.target.value || null })} />
              </label>
            </div>

            <div className="tabs tabs--editor" role="tablist">
              {doc.lyrics.map((l, i) => (
                <button key={l.lang} role="tab" aria-selected={i === tab} onClick={() => setTab(i)}>
                  {langName(l.lang)}
                  {i === 0 ? "" : " (second)"}
                </button>
              ))}
              {doc.lyrics.length < 2 && (
                <select
                  className="add-lang"
                  value=""
                  aria-label="Add a second language"
                  onChange={(e) => {
                    if (!e.target.value) return;
                    const labels = doc.lyrics[0].body.match(/^\[.+\]$/gm) ?? ["[V1]"];
                    update({ lyrics: [...doc.lyrics, { lang: e.target.value, body: labels.join("\n\n") + "\n" }] });
                    setTab(doc.lyrics.length);
                  }}
                >
                  <option value="">Add a language…</option>
                  {unused.map((l) => (
                    <option key={l.code} value={l.code}>
                      {l.name}
                    </option>
                  ))}
                </select>
              )}
              {tab > 0 && (
                <button
                  className="btn-quiet tabs__end"
                  onClick={() => {
                    if (!confirm(`Remove the ${langName(doc.lyrics[tab].lang)} lyrics from this song?`)) return;
                    update({ lyrics: doc.lyrics.filter((_, i) => i !== tab) });
                    setTab(0);
                  }}
                >
                  Remove {langName(doc.lyrics[tab].lang)}
                </button>
              )}
            </div>

            <div className="editor__tools">
              <button className="btn-quiet" onClick={() => insertSection("V")}>
                + Verse
              </button>
              <button className="btn-quiet" onClick={() => insertSection("C")}>
                + Chorus
              </button>
              <button className="btn-quiet" onClick={() => insertSection("B")}>
                + Bridge
              </button>
              <select
                className="wp-input editor__tidy"
                value=""
                aria-label="Tidy the words"
                title="Tidy the words in this language"
                onChange={(e) => {
                  const f = TIDY[e.target.value];
                  if (f) update({ lyrics: doc.lyrics.map((l, i) => (i === tab ? { ...l, body: f(l.body) } : l)) });
                }}
              >
                <option value="">Tidy…</option>
                <option value="caps">Capitalise the first letter of each line</option>
                <option value="ends">Remove punctuation at line ends</option>
                <option value="spaces">Remove extra spaces</option>
                <option value="blank">Remove blank lines inside sections</option>
              </select>
              <span className="editor__sep" />
              <button className="char-btn" style={{ fontWeight: 800 }} onClick={() => wrap("**", "**")} title="Bold: select words, then click (shown on the screens)">
                B
              </button>
              <button className="char-btn" style={{ fontStyle: "italic" }} onClick={() => wrap("*", "*")} title="Italic">
                I
              </button>
              <button className="char-btn" style={{ textDecoration: "underline" }} onClick={() => wrap("__", "__")} title="Underline">
                U
              </button>
              <select
                className="wp-input editor__tidy"
                value=""
                aria-label="Colour the selected words"
                title="Colour the selected words"
                onChange={(e) => e.target.value && wrap(`{${e.target.value}}`, "{/}")}
              >
                <option value="">Colour…</option>
                {["gold", "yellow", "orange", "red", "pink", "purple", "blue", "green", "white", "silver"].map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
              <span className="editor__sep" />
              {CHARS.map((c) => (
                <button key={c} className="char-btn" onClick={() => insert(c)} title={`Type ${c}`}>
                  {c}
                </button>
              ))}
            </div>
            <textarea
              ref={area}
              className="lyrics"
              // Windows' spell check for English words; off for Ghanaian languages,
              // which it has no dictionary for (everything would be underlined)
              spellCheck={doc.lyrics[tab].lang.startsWith("en")}
              lang={doc.lyrics[tab].lang}
              value={doc.lyrics[tab].body}
              onChange={(e) => setBody(e.target.value)}
              aria-label={`${langName(doc.lyrics[tab].lang)} lyrics`}
            />
            <p className="hint hint--tight">
              Start each part with a label in square brackets: [V1], [V2], [C] for chorus, [B] for bridge. The second language uses the same labels; its lines are paired with the first
              language part by part.
            </p>
            <div className="fields fields--two">
              <label className="field">
                <span>Play order</span>
                <input value={doc.sequence} placeholder="As written. Or e.g. V1 C V2 C" onChange={(e) => update({ sequence: e.target.value })} />
              </label>
              <div className="field field--wide">
                <ArrangementEditor sections={sectionsOf(doc.lyrics[0]?.body ?? "")} order={doc.sequence} onChange={(sequence) => update({ sequence })} />
              </div>
              <label className="field">
                <span>Lines per slide</span>
                <select value={doc.lines_per_slide ?? ""} onChange={(e) => update({ lines_per_slide: e.target.value ? Number(e.target.value) : null })}>
                  <option value="">Use the Live setting</option>
                  {[1, 2, 3, 4, 5, 6].map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </div>

          <div className="editor__preview" aria-label="Slide preview">
            <div className="editor__preview-head">
              <div className="wp-tabs" role="tablist" aria-label="Right side">
                <button role="tab" aria-selected={view === "slides"} onClick={() => setView("slides")}>
                  Slides
                </button>
                <button role="tab" aria-selected={view === "pairing"} onClick={() => setView("pairing")} disabled={doc.lyrics.length < 2}>
                  Pairing {pairs.unpaired ? `(${pairs.unpaired} to check)` : ""}
                </button>
              </div>
              <span className="wp-spacer" />
              {problem ? <span className="editor__problem">{problem}</span> : <span className="hint hint--inline">{slides.length === 1 ? "1 slide" : `${slides.length} slides`}</span>}
            </div>
            {view === "slides" || doc.lyrics.length < 2 ? (
              <div className="grid grid--editor">
                {slides.map((s, i) => (
                  <div key={i} className="thumb">
                    <span className="thumb__label">{s.label}</span>
                    <SlideView slide={s} />
                  </div>
                ))}
              </div>
            ) : (
              <div className="pairing">
                <p className="hint hint--tight">
                  Each {langName(doc.lyrics[0].lang)} line and the {langName(doc.lyrics[1].lang)} line beside it show together. Where a part has a different number of lines, the
                  lines are shared out evenly; add or remove a line to pair them one to one.
                </p>
                {pairs.rows.map((sec) => (
                  <section key={sec.label} className={`pairing__sec ${sec.uneven ? "is-uneven" : ""}`}>
                    <header>
                      <strong>[{sec.label}]</strong>
                      {sec.uneven && (
                        <>
                          <span className="pairing__flag">
                            {sec.a} {langName(doc.lyrics[0].lang)} lines, {sec.b} {langName(doc.lyrics[1].lang)} lines
                          </span>
                          <button className="btn-quiet" onClick={() => fixIn(sec.a > sec.b ? 1 : 0, sec.label)}>
                            Fix in {langName(doc.lyrics[sec.a > sec.b ? 1 : 0].lang)}
                          </button>
                        </>
                      )}
                      {sec.missing && <span className="pairing__flag">No {langName(doc.lyrics[1].lang)} for this part</span>}
                    </header>
                    {sec.lines.map(([x, y], i) => (
                      <div key={i} className="pairing__row">
                        <span>{x}</span>
                        <span lang={doc.lyrics[1].lang}>{y}</span>
                      </div>
                    ))}
                  </section>
                ))}
              </div>
            )}
          </div>
        </div>
        <div className="dialog__foot">
          {doc.id != null && (
            <button className="btn-quiet btn-danger" onClick={remove}>
              Delete song
            </button>
          )}
          <span className="topbar__spacer" />
          <button className="btn-quiet" onClick={close}>
            Cancel
          </button>
          {onApply && (
            <>
              {serviceItem?.lyrics && (
                <button className="btn-quiet" onClick={() => (onApply(null), setDirty(false), onClose())} title="Use the library words for this service again">
                  Use library words
                </button>
              )}
              <button
                className="btn-quiet"
                disabled={!!problem}
                onClick={() => (onApply(full.lyrics), setDirty(false), onClose())}
                title="Use these words in this service only. The library and other services keep theirs."
              >
                Apply to this service
              </button>
            </>
          )}
          <button className="wp-btn wp-btn--primary se-save" onClick={save} disabled={saving || !!problem || !doc.title.trim()} title="Save for every service. Never changes what is on the screens.">
            {saving ? "Saving…" : "Save to library"} <kbd>Ctrl+S</kbd>
          </button>
        </div>
      </div>
    </div>
  );
}
