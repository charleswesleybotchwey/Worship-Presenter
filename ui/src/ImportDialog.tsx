import { useState } from "react";
import { api, pickPath, pickSongFiles, type ImportKind, type ImportReport, type ImportReview } from "./api";

// Import (design brief §5): choose source → review → commit → report. The review
// runs against a scratch copy, so nothing is written until Import is chosen.

const SOURCES: { kind: ImportKind; title: string; detail: string; code: boolean }[] = [
  { kind: "ew-songs", title: "EasyWorship songs", detail: "The folder that holds Songs.db and SongWords.db", code: false },
  { kind: "ewb", title: "EasyWorship Bible", detail: "An .ewb file", code: true },
  { kind: "bibleshow", title: "Bible Show Bible", detail: "A .bib file", code: true },
];

const n = (x: number) => x.toLocaleString();

type Kind = ImportKind | "song-files";
type Step =
  | { at: "choose" }
  | { at: "checking"; kind: Kind; path: string }
  | { at: "review"; kind: Kind; path: string; paths?: string[]; review: ImportReview }
  | { at: "importing"; kind: Kind }
  | { at: "done"; report: ImportReport };

export function ImportDialog({ onClose }: { onClose: () => void }) {
  const [step, setStep] = useState<Step>({ at: "choose" });
  const [code, setCode] = useState("");
  const [skipDupes, setSkipDupes] = useState(true);
  const [open, setOpen] = useState<"ambiguous" | "rejected" | "duplicates" | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const busy = step.at === "checking" || step.at === "importing";

  const checkFiles = async (paths: string[]) => {
    if (!paths.length) return;
    setProblem(null);
    setStep({ at: "checking", kind: "song-files", path: paths[0] });
    try {
      setStep({ at: "review", kind: "song-files", path: paths[0], paths, review: await api.songFilesReview(paths) });
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
      setStep({ at: "choose" });
    }
  };

  const check = async (kind: ImportKind, path: string) => {
    setProblem(null);
    setStep({ at: "checking", kind, path });
    try {
      setStep({ at: "review", kind, path, review: await api.importReview(kind, path, code.trim() || null) });
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
      setStep({ at: "choose" });
    }
  };

  const commit = async () => {
    if (step.at !== "review") return;
    const { kind, path, review } = step;
    setStep({ at: "importing", kind });
    try {
      const songs = kind === "ew-songs" || kind === "song-files";
      const skip = songs && skipDupes ? review.duplicates : null;
      const report = kind === "song-files" ? await api.songFilesImport(step.paths ?? [path], skip) : await api.importFile(kind, path, code.trim() || null, skip);
      setStep({ at: "done", report });
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
      setStep({ at: "review", kind, path, review });
    }
  };

  const bibleClash = step.at === "review" && step.kind !== "ew-songs" && step.review.duplicates.length > 0;

  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && !busy && onClose()}>
      <div className="wp-dialog" role="dialog" aria-modal="true" aria-label="Import">
        <header className="wp-dialog__head">
          <h2>{step.at === "choose" || step.at === "checking" ? "Import: choose a source" : step.at === "review" ? `Import: review ${step.review.name}` : "Import"}</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose} disabled={busy} autoFocus>
            {step.at === "done" ? "Done" : "Cancel"}
          </button>
        </header>

        {(step.at === "choose" || step.at === "checking") && (
          <>
            {SOURCES.map((s) => (
              <div className="wp-setting wp-setting--two" key={s.kind}>
                <div>
                  <div className="wp-setting__name">{s.title}</div>
                  <div className="wp-setting__hint">{s.detail}</div>
                </div>
                <button
                  className="wp-btn"
                  disabled={busy}
                  onClick={async () => {
                    const p = await pickPath(s.kind);
                    if (p) void check(s.kind, p);
                  }}
                >
                  {step.at === "checking" && step.kind === s.kind ? "Checking…" : "Choose…"}
                </button>
              </div>
            ))}
            <div className="wp-setting wp-setting--two">
              <div>
                <div className="wp-setting__name">Song files from other programs</div>
                <div className="wp-setting__hint">OpenLyrics (OpenLP), OpenSong, ProPresenter 6 and 7, CCLI SongSelect, ChordPro, plain text</div>
              </div>
              <span className="wp-row-inline">
                <button className="wp-btn" disabled={busy} onClick={async () => checkFiles(await pickSongFiles(false))}>
                  {step.at === "checking" && step.kind === "song-files" ? "Checking…" : "Files…"}
                </button>
                <button className="wp-btn wp-btn--quiet" disabled={busy} onClick={async () => checkFiles(await pickSongFiles(true))}>
                  Folder…
                </button>
              </span>
            </div>
            <p className="wp-note">Nothing is added until you have seen what the files contain and chosen Import.</p>
          </>
        )}

        {step.at === "review" && (
          <>
            <dl className="review">
              <div>
                <dt>Recognised</dt>
                <dd className="wp-num">{n(step.review.recognised)}</dd>
              </div>
              <div>
                <dt>Changed on the way</dt>
                <dd className="wp-num">
                  {n(step.review.ambiguous.length)}
                  {step.review.ambiguous.length > 0 && (
                    <button className="wp-btn wp-btn--quiet" onClick={() => setOpen(open === "ambiguous" ? null : "ambiguous")}>
                      {open === "ambiguous" ? "Hide" : "Show"}
                    </button>
                  )}
                </dd>
              </div>
              <div>
                <dt>Rejected</dt>
                <dd className="wp-num">
                  {n(step.review.rejected.length)}
                  {step.review.rejected.length > 0 && (
                    <button className="wp-btn wp-btn--quiet" onClick={() => setOpen(open === "rejected" ? null : "rejected")}>
                      {open === "rejected" ? "Hide" : "Show"}
                    </button>
                  )}
                </dd>
              </div>
              <div>
                <dt>Already in this library</dt>
                <dd className="wp-num">
                  {n(step.review.duplicates.length)}
                  {step.review.duplicates.length > 0 && (
                    <button className="wp-btn wp-btn--quiet" onClick={() => setOpen(open === "duplicates" ? null : "duplicates")}>
                      {open === "duplicates" ? "Hide" : "Show"}
                    </button>
                  )}
                </dd>
              </div>
            </dl>
            {open && (
              <ul className="review__list">
                {open === "duplicates"
                  ? step.review.duplicates.map((d) => <li key={d}>{d}</li>)
                  : step.review[open].map((x, i) => (
                      <li key={i}>
                        <strong>{x.item}</strong> {x.reason}
                      </li>
                    ))}
              </ul>
            )}
            {open === "ambiguous" && <p className="wp-note">Look-alike characters from old fonts were changed to the real letters (for example Greek ε to ɛ). Check a few of these songs after importing.</p>}
            {(step.kind === "ew-songs" || step.kind === "song-files") && step.review.duplicates.length > 0 && (
              <label className="wp-check">
                <input type="checkbox" checked={skipDupes} onChange={(e) => setSkipDupes(e.target.checked)} />
                Leave out the {n(step.review.duplicates.length)} songs already in this library
              </label>
            )}
            {bibleClash && (
              <div className="wp-setting wp-setting--two">
                <div>
                  <div className="wp-setting__name">This Bible's short code “{step.review.duplicates[0]}” is already used</div>
                  <div className="wp-setting__hint">Give this one a different code, then check again.</div>
                </div>
                <span className="wp-row-inline">
                  <input className="wp-input" value={code} placeholder="e.g. TK-2" onChange={(e) => setCode(e.target.value)} aria-label="Short code" />
                  <button className="wp-btn" onClick={() => step.kind !== "song-files" && void check(step.kind, step.path)} disabled={!code.trim()}>
                    Check again
                  </button>
                </span>
              </div>
            )}
            {step.review.notes.map((x) => (
              <p className="wp-note" key={x}>
                {x}
              </p>
            ))}
            <footer className="wp-dialog__foot">
              <button className="wp-btn wp-btn--quiet" onClick={() => setStep({ at: "choose" })}>
                Back
              </button>
              <button className="wp-btn wp-btn--primary" onClick={commit} disabled={bibleClash}>
                Import {step.kind === "ew-songs" || step.kind === "song-files" ? `${n(step.review.read - (skipDupes ? step.review.duplicates.length : 0) - step.review.rejected.length)} songs` : "Bible"}
              </button>
            </footer>
          </>
        )}

        {step.at === "importing" && <p className="wp-note">Importing… This can take a minute for a large library. The screens keep working.</p>}

        {step.at === "done" && (
          <div className="review review--done">
            <p>
              <strong>{step.report.name}</strong>: {n(step.report.read)} read, {n(step.report.imported)} imported
              {step.report.skipped ? `, ${n(step.report.skipped)} left out` : ""}
              {step.report.fixed_chars ? `, ${n(step.report.fixed_chars)} characters corrected` : ""}.
            </p>
            {step.report.notes.map((x) => (
              <p className="wp-note" key={x}>
                {x}
              </p>
            ))}
            <p className="wp-note">A backup was taken before importing. Every change is listed in the import log.</p>
          </div>
        )}
        {problem && <p className="wp-banner wp-banner--bad">{problem}</p>}
      </div>
    </div>
  );
}
