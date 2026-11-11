import { useMemo, useState } from "react";
import { api, type ForeignDesign } from "../api";
import { ThemeView } from "./ThemeView";
import type { Theme } from "./types";

// EasyWorship (.ewtx) and ProPresenter (.proTheme) themes: every design shown as it
// will look, with its detected use; the operator ticks which ones to bring in.

const KIND: Record<string, string> = { song: "Songs", scripture: "Scripture", lower_third: "Lower third", announcement: "Announcements" };

const SAMPLE: Record<string, { primary: string[]; second: string[]; reference?: string; title?: string; section?: string }> = {
  song: { primary: ["O for a thousand tongues to sing", "My great Redeemer’s praise,"], second: ["Meenya ngyiramá apem matow ndwom"], section: "Verse 1", title: "MHB 1" },
  scripture: { primary: ["For God so loved the world that He gave His only begotten Son"], second: [], reference: "John 3:16" },
  lower_third: { primary: ["O for a thousand tongues to sing"], second: ["Meenya ngyiramá apem matow ndwom"], reference: "John 3:16" },
  announcement: { primary: ["Harvest Thanksgiving, Sunday 15 November"], second: [], title: "Announcements" },
};

export function ForeignThemeImport({ path, designs, onDone, onCancel }: { path: string; designs: ForeignDesign[]; onDone: (msg: string, firstId?: number) => void; onCancel: () => void }) {
  const [chosen, setChosen] = useState<Set<number>>(() => new Set(designs.map((d, i) => (d.suggested ? i : -1)).filter((i) => i >= 0)));
  const [filter, setFilter] = useState<string>("all");
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const shown = useMemo(() => designs.map((d, i) => ({ d, i })).filter(({ d }) => filter === "all" || d.kind === filter), [designs, filter]);
  const toggle = (i: number) => setChosen((s) => (s.has(i) ? new Set([...s].filter((x) => x !== i)) : new Set([...s, i])));
  const source = path.toLowerCase().endsWith(".ewtx") ? "EasyWorship" : "ProPresenter";
  const go = async () => {
    setBusy(true);
    try {
      const r = await api.foreignThemeImport(path, [...chosen].sort((a, b) => a - b));
      onDone(`Imported ${r.length} theme${r.length === 1 ? "" : "s"} from ${source}. Pictures went into the media library. Check fonts: a font that is not installed on this computer is replaced by Noto Sans.`, r[0]?.id);
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
      setBusy(false);
    }
  };
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <div className="wp-dialog wp-dialog--wide ft" role="dialog" aria-modal="true" aria-label={`Themes from ${source}`}>
        <header className="wp-dialog__head">
          <h2>
            Themes from {source} <span className="wp-note wp-note--inline">{designs.length} design{designs.length === 1 ? "" : "s"}</span>
          </h2>
          <button className="wp-btn wp-btn--quiet" onClick={onCancel}>
            Cancel
          </button>
        </header>
        <div className="ft-bar">
          <div className="wp-tabs" role="tablist">
            {["all", "song", "scripture", "lower_third", "announcement"]
              .filter((k) => k === "all" || designs.some((d) => d.kind === k))
              .map((k) => (
                <button key={k} role="tab" aria-selected={filter === k} onClick={() => setFilter(k)}>
                  {k === "all" ? "All" : KIND[k]} <span className="wp-num">{k === "all" ? designs.length : designs.filter((d) => d.kind === k).length}</span>
                </button>
              ))}
          </div>
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--quiet" onClick={() => setChosen(new Set([...chosen, ...shown.map((x) => x.i)]))}>
            Tick all shown
          </button>
          <button className="wp-btn wp-btn--quiet" onClick={() => setChosen(new Set([...chosen].filter((i) => !shown.some((x) => x.i === i))))}>
            Untick all shown
          </button>
        </div>
        <div className="ft-grid">
          {shown.map(({ d, i }) => (
            <label key={i} className={`ft-card ${chosen.has(i) ? "is-on" : ""}`} title={d.notes.join("\n")}>
              <div className="ft-pic">
                <ThemeView theme={d.preview as unknown as Theme} data={SAMPLE[d.kind] ?? SAMPLE.song} still />
              </div>
              <span className="ft-name">
                <input type="checkbox" checked={chosen.has(i)} onChange={() => toggle(i)} />
                <span>{d.name}</span>
              </span>
              <span className="ft-kind">{KIND[d.kind] ?? d.kind}</span>
            </label>
          ))}
        </div>
        <footer className="ft-foot">
          <span className="wp-note wp-note--inline">
            Positions, fonts, colours, boxes, outlines and pictures come across. Hover a design to see what could not. {problem && <strong className="form-error">{problem}</strong>}
          </span>
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--primary" disabled={!chosen.size || busy} onClick={go}>
            {busy ? "Importing…" : `Import ${chosen.size} theme${chosen.size === 1 ? "" : "s"}`}
          </button>
        </footer>
      </div>
    </div>
  );
}
