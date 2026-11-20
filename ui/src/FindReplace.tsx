import { useState } from "react";
import { api, type ReplaceOptions } from "./api";

// Find and replace across every song: preview the changes, then make them (a backup is
// made first). Section marks such as [V1] are never touched.

export function FindReplace({ onClose, onDone }: { onClose: () => void; onDone: (n: number) => void }) {
  const [find, setFind] = useState("");
  const [withText, setWith] = useState("");
  const [o, setO] = useState<ReplaceOptions>({ match_case: false, whole_words: true, in_words: true, in_titles: false });
  const [plan, setPlan] = useState<{ song_id: number; title: string; count: number; before: string; after: string }[] | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const opt = (k: keyof ReplaceOptions, label: string) => (
    <label className="wp-check">
      <input type="checkbox" checked={o[k]} onChange={(e) => (setO({ ...o, [k]: e.target.checked }), setPlan(null))} /> {label}
    </label>
  );
  const preview = async () => {
    try {
      setPlan(await api.songsReplacePlan(find, withText, o));
      setProblem(null);
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };
  const total = plan?.reduce((a, c) => a + c.count, 0) ?? 0;
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog wp-dialog--wide frp" role="dialog" aria-modal="true" aria-label="Find and replace in songs">
        <header className="wp-dialog__head">
          <h2>Find and replace in songs</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        <div className="frp__body">
          <label className="ps__row">
            <span>Find</span>
            <input className="wp-input" value={find} onChange={(e) => (setFind(e.target.value), setPlan(null))} aria-label="Find" autoFocus />
          </label>
          <label className="ps__row">
            <span>Replace with</span>
            <input className="wp-input" value={withText} onChange={(e) => (setWith(e.target.value), setPlan(null))} aria-label="Replace with" />
          </label>
          <div className="frp__opts">
            {opt("whole_words", "Whole words only")}
            {opt("match_case", "Match case")}
            {opt("in_words", "In the words")}
            {opt("in_titles", "In the titles")}
          </div>
          <button className="wp-btn" disabled={!find.trim() || (!o.in_words && !o.in_titles)} onClick={() => void preview()}>
            Preview the changes
          </button>
          {plan && (
            <>
              <p className="wp-note">{plan.length === 0 ? "Not found in any song." : `${total} change${total === 1 ? "" : "s"} in ${plan.length} song${plan.length === 1 ? "" : "s"}:`}</p>
              <ul className="frp__list">
                {plan.slice(0, 200).map((c) => (
                  <li key={c.song_id}>
                    <strong>{c.title}</strong> <span className="wp-note wp-note--inline">({c.count})</span>
                    <div className="frp__diff">
                      <span className="frp__before">{c.before}</span> → <span className="frp__after">{c.after}</span>
                    </div>
                  </li>
                ))}
              </ul>
              {plan.length > 0 && (
                <button className="wp-btn wp-btn--primary" onClick={() => void api.songsReplaceApply(find, withText, o).then(onDone, (e) => setProblem(String(e)))}>
                  Replace in {plan.length} song{plan.length === 1 ? "" : "s"}
                </button>
              )}
            </>
          )}
          {problem && <p className="form-error">{problem}</p>}
          <p className="wp-note">A backup is made before anything changes. Section marks such as [V1] are never touched.</p>
        </div>
      </div>
    </div>
  );
}
