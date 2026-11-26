import { useEffect, useState } from "react";
import { api } from "../api";
import { ThemeView } from "./ThemeView";
import type { Theme } from "./types";

// Ready-made themes for Ghanaian church use. They use the brand kit, so they appear in
// the church's own colours and fonts (Settings → Brand kit).

const KIND: Record<string, string> = { song: "Songs", scripture: "Scripture", lower_third: "Lower third", announcement: "Announcements" };
/** Sample words for a theme's picture, by kind (a scripture theme shows a verse…). */
export const SAMPLE: Record<string, { primary: string[]; second: string[]; reference?: string; title?: string; section?: string; first?: boolean; last?: boolean }> = {
  song: { primary: ["O for a thousand tongues to sing", "My great Redeemer’s praise,"], second: ["Meenya ngyiramá apem matow ndwom", "Meyi mo Pomfò ayɛw,"], title: "MHB 1", section: "Verse 1", first: true, last: false },
  scripture: { primary: ["For God so loved the world that He gave His only begotten Son"], second: [], reference: "John 3:16" },
  lower_third: { primary: ["O for a thousand tongues to sing"], second: ["Meenya ngyiramá apem matow ndwom"], reference: "MHB 1" },
  announcement: { primary: ["Harvest Thanksgiving: Sunday 15 November, 9 am. Bring your harvest offering."], second: [], title: "Harvest" },
};

export function Gallery({ onClose, onAdded }: { onClose: () => void; onAdded: (name: string) => void }) {
  const [themes, setThemes] = useState<Theme[]>([]);
  const [busy, setBusy] = useState<number | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    api.themeGallery().then(setThemes).catch((e) => setProblem(String(e)));
  }, []);
  const add = async (i: number, use: boolean) => {
    setBusy(i);
    try {
      const t = themes[i];
      const id = await api.saveTheme(null, t);
      if (use) await api.setDefaultTheme(t.kind, id);
      onAdded(t.name);
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(null);
    }
  };
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog wp-dialog--wide ft" role="dialog" aria-modal="true" aria-label="Theme gallery">
        <header className="wp-dialog__head">
          <h2>Theme gallery</h2>
          <span className="wp-note wp-note--inline">Shown in your brand colours and fonts (Settings → Brand kit). Add one, then change anything in the editor.</span>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        <div className="ft-grid">
          {themes.map((t, i) => (
            <div key={i} className="ft-card gal-card">
              <div className="ft-pic">
                <ThemeView theme={t} data={SAMPLE[t.kind] ?? SAMPLE.song} still />
              </div>
              <span className="ft-name">{t.name}</span>
              <span className="ft-kind">{KIND[t.kind] ?? t.kind}</span>
              <span className="gal-actions">
                <button className="wp-btn" disabled={busy !== null} onClick={() => void add(i, false)}>
                  Add
                </button>
                <button className="wp-btn wp-btn--quiet" disabled={busy !== null} title={`Add it and use it for every ${(KIND[t.kind] ?? t.kind).toLowerCase()} item`} onClick={() => void add(i, true)}>
                  Add and use
                </button>
              </span>
            </div>
          ))}
        </div>
        {problem && <p className="form-error">{problem}</p>}
      </div>
    </div>
  );
}
