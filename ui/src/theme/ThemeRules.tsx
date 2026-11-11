import { useEffect, useState } from "react";
import { api, type ThemeRule } from "../api";
import type { ThemeRow } from "./types";

// Theme rules: "For songs when the hymnbook is MHB, use BLACK." The first rule that
// matches wins; an item's own theme and an output's theme still come first.

const MATCHES: Record<"song" | "scripture", { value: ThemeRule["match"]; label: string }[]> = {
  song: [
    { value: "hymnal", label: "the hymnbook is" },
    { value: "language", label: "it has the language" },
    { value: "bilingual", label: "it has two languages" },
    { value: "interval", label: "the service interval is" },
  ],
  scripture: [
    { value: "version", label: "the Bible is" },
    { value: "bilingual", label: "two Bibles are shown" },
    { value: "interval", label: "the service interval is" },
  ],
};

const LANGS = [
  ["en", "English"],
  ["fat", "Fante"],
  ["tw", "Twi"],
  ["gaa", "Ga"],
  ["ee", "Ewe"],
  ["dag", "Dagbani"],
  ["fr", "French"],
];

export function ThemeRules({ onClose }: { onClose: (saved: boolean) => void }) {
  const [rules, setRules] = useState<ThemeRule[]>([]);
  const [themes, setThemes] = useState<ThemeRow[]>([]);
  const [hymnals, setHymnals] = useState<string[]>([]);
  const [intervals, setIntervals] = useState<string[]>([]);
  const [bibles, setBibles] = useState<string[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    api.themeRules().then(setRules).catch(() => {});
    api.listThemes().then(setThemes).catch(() => {});
    api.songTree().then((t) => setHymnals(t.collections.map((c) => c.key.split(":").pop() ?? c.name))).catch(() => {});
    api.serviceIntervals().then((i) => setIntervals(i.map((x) => x.name))).catch(() => {});
    api.versions().then((v) => setBibles(v.map((x) => x.code))).catch(() => {});
  }, []);
  const set = (i: number, p: Partial<ThemeRule>) => setRules((rs) => rs.map((r, j) => (j === i ? { ...r, ...p } : r)));
  const move = (i: number, d: number) =>
    setRules((rs) => {
      const n = [...rs];
      const j = i + d;
      if (j < 0 || j >= n.length) return rs;
      [n[i], n[j]] = [n[j], n[i]];
      return n;
    });
  const themesFor = (kind: string) => themes.filter((t) => t.kind === kind);
  const add = () => {
    const t = themesFor("song")[0];
    setRules((rs) => [...rs, { id: 0, kind: "song", match: "hymnal", value: hymnals[0] ?? "", theme_id: t?.id ?? 0 }]);
  };
  const save = async () => {
    try {
      await api.saveThemeRules(rules.map((r) => ({ ...r, value: r.match === "bilingual" ? r.value || "yes" : r.value })));
      onClose(true);
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose(false)}>
      <div className="wp-dialog wp-dialog--wide tr" role="dialog" aria-modal="true" aria-label="Theme rules">
        <header className="wp-dialog__head">
          <h2>Theme rules</h2>
          <span className="wp-note wp-note--inline">The first rule that fits is used. A theme chosen for one item, or for one output, still comes first.</span>
        </header>
        <div className="tr__list">
          {rules.length === 0 && <p className="wp-note">No rules yet: every song and reading uses its default theme.</p>}
          {rules.map((r, i) => (
            <div key={i} className="tr__row">
              <span className="wp-num tr__n">{i + 1}</span>
              For
              <select value={r.kind} aria-label="Kind" onChange={(e) => set(i, { kind: e.target.value as ThemeRule["kind"], match: MATCHES[e.target.value as "song"][0].value, theme_id: themesFor(e.target.value)[0]?.id ?? 0 })}>
                <option value="song">songs</option>
                <option value="scripture">scripture</option>
              </select>
              when
              <select value={r.match} aria-label="When" onChange={(e) => set(i, { match: e.target.value as ThemeRule["match"], value: e.target.value === "bilingual" ? "yes" : "" })}>
                {MATCHES[r.kind].map((m) => (
                  <option key={m.value} value={m.value}>
                    {m.label}
                  </option>
                ))}
              </select>
              {r.match === "bilingual" ? (
                <select value={r.value || "yes"} aria-label="Two languages" onChange={(e) => set(i, { value: e.target.value })}>
                  <option value="yes">yes</option>
                  <option value="no">no (one only)</option>
                </select>
              ) : r.match === "language" ? (
                <select value={r.value} aria-label="Language" onChange={(e) => set(i, { value: e.target.value })}>
                  <option value="">choose…</option>
                  {LANGS.map(([c, n]) => (
                    <option key={c} value={c}>
                      {n}
                    </option>
                  ))}
                </select>
              ) : (
                <select value={r.value} aria-label="Value" onChange={(e) => set(i, { value: e.target.value })}>
                  <option value="">choose…</option>
                  {(r.match === "hymnal" ? hymnals : r.match === "interval" ? intervals : bibles).map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              )}
              , use
              <select value={r.theme_id} aria-label="Theme" onChange={(e) => set(i, { theme_id: Number(e.target.value) })}>
                {themesFor(r.kind).map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
              </select>
              <span className="wp-spacer" />
              <button className="wp-icon-btn" aria-label="Move up" onClick={() => move(i, -1)} disabled={i === 0}>
                ↑
              </button>
              <button className="wp-icon-btn" aria-label="Move down" onClick={() => move(i, 1)} disabled={i === rules.length - 1}>
                ↓
              </button>
              <button className="wp-icon-btn" aria-label="Remove rule" onClick={() => setRules((rs) => rs.filter((_, j) => j !== i))}>
                ✕
              </button>
            </div>
          ))}
        </div>
        <footer className="tr__foot">
          <button className="wp-btn" onClick={add}>
            Add a rule
          </button>
          {problem && <span className="form-error">{problem}</span>}
          <span className="wp-spacer" />
          <button className="wp-btn wp-btn--quiet" onClick={() => onClose(false)}>
            Cancel
          </button>
          <button className="wp-btn wp-btn--primary" onClick={save}>
            Save rules
          </button>
        </footer>
      </div>
    </div>
  );
}
