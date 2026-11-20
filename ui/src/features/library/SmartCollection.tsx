import { useEffect, useState } from "react";
import { api } from "../../api";

// A smart song collection, as EasyWorship's "Edit Song Collection": match all / any of
// rules such as "Author is Charles Wesley". It lists the matching songs and keeps itself
// up to date as songs are added.

type Rule = { field: string; op: string; value: string };
const FIELDS: [string, string][] = [
  ["title", "Title"],
  ["author", "Author"],
  ["copyright", "Copyright"],
  ["ccli", "CCLI number"],
  ["words", "Words"],
  ["any", "Any field"],
];
const OPS: [string, string][] = [
  ["contains", "contains"],
  ["not_contains", "does not contain"],
  ["is", "is"],
  ["is_not", "is not"],
  ["starts", "starts with"],
  ["ends", "ends with"],
];

export function SmartCollection({ id, name: start, onClose, onSaved }: { id: number | null; name: string; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState(start);
  const [all, setAll] = useState(true);
  const [rules, setRules] = useState<Rule[]>([{ field: "title", op: "contains", value: "" }]);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    if (id != null)
      api
        .smartRules(id)
        .then((r) => r && (setAll(r.all), setRules(r.rules.length ? r.rules : [{ field: "title", op: "contains", value: "" }])))
        .catch(() => {});
  }, [id]);
  const set = (i: number, p: Partial<Rule>) => setRules((rs) => rs.map((r, k) => (k === i ? { ...r, ...p } : r)));
  const save = async () => {
    if (!name.trim()) return setProblem("Give the collection a name.");
    if (!rules.some((r) => r.value.trim())) return setProblem("Fill in at least one rule.");
    try {
      const cid = id ?? (await api.collectionCreate(name));
      if (id != null && name.trim() !== start) await api.collectionRename(id, name);
      await api.smartRulesSet(cid, { all, rules: rules.filter((r) => r.value.trim()) });
      onSaved();
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog smc" role="dialog" aria-modal="true" aria-label="Song collection rules">
        <header className="wp-dialog__head">
          <h2>{id == null ? "New smart collection" : "Edit collection rules"}</h2>
        </header>
        <div className="smc__body">
          <label className="ps__row">
            <span>Name</span>
            <input className="wp-input" value={name} onChange={(e) => setName(e.target.value)} aria-label="Name" autoFocus />
          </label>
          <div className="smc__match">
            Match
            <select className="wp-input" value={all ? "all" : "any"} onChange={(e) => setAll(e.target.value === "all")} aria-label="Match">
              <option value="all">All</option>
              <option value="any">Any</option>
            </select>
            of the following rules:
          </div>
          <div className="smc__rules">
            {rules.map((r, i) => (
              <div key={i} className="smc__rule">
                <select className="wp-input" value={r.field} onChange={(e) => set(i, { field: e.target.value })} aria-label="Field">
                  {FIELDS.map(([v, l]) => (
                    <option key={v} value={v}>
                      {l}
                    </option>
                  ))}
                </select>
                <select className="wp-input" value={r.op} onChange={(e) => set(i, { op: e.target.value })} aria-label="Comparison">
                  {OPS.map(([v, l]) => (
                    <option key={v} value={v}>
                      {l}
                    </option>
                  ))}
                </select>
                <input className="wp-input" value={r.value} onChange={(e) => set(i, { value: e.target.value })} aria-label="Value" />
                <button className="wp-icon-btn" aria-label="Remove rule" disabled={rules.length === 1} onClick={() => setRules((rs) => rs.filter((_, k) => k !== i))}>
                  −
                </button>
                <button className="wp-icon-btn" aria-label="Add rule" onClick={() => setRules((rs) => [...rs.slice(0, i + 1), { field: "title", op: "contains", value: "" }, ...rs.slice(i + 1)])}>
                  +
                </button>
              </div>
            ))}
          </div>
          {problem && <p className="form-error">{problem}</p>}
          <div className="ps__foot">
            <button className="wp-btn" onClick={onClose}>
              Cancel
            </button>
            <button className="wp-btn wp-btn--primary" onClick={() => void save()}>
              OK
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
