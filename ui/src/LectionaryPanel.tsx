import { useEffect, useState } from "react";
import { api, type Lectionary, type ProgramItem } from "./api";

// The lectionary: the day (its name, year and season) and the church's readings, ready to
// go into the service.

const SEASONS: Record<string, string> = { advent: "Advent", christmas: "Christmas", epiphany: "Epiphany", lent: "Lent", easter: "Easter", pentecost: "after Pentecost" };

/** "Isaiah 64:1-64:9" → "Isaiah 64:1-9"; the first of alternatives; optional verses dropped. */
export function cleanReading(r: string): string {
  let s = r.split(/\s+or\s+/i)[0];
  s = s.replace(/,?\s*\([^)]*\)/g, "").trim();
  s = s.replace(/(\d+):(\d+[a-z]?)-(\d+):(\d+[a-z]?)/g, (m, c1, v1, c2, v2) => (c1 === c2 ? `${c1}:${v1}-${v2}` : m));
  // later parts in the same chapter lose the repeated chapter: "80:1-7, 80:16-18" → "80:1-7, 16-18"
  const chapter = /(\d+):\d/.exec(s)?.[1];
  if (chapter) s = s.split(/,\s*/).map((part, i) => (i > 0 && part.startsWith(`${chapter}:`) ? part.slice(chapter.length + 1) : part)).join(", ");
  return s.replace(/(\d+)[ab]\b/g, "$1").trim();
}

/** A reading as references the app reads: "Psalm 80:1-7, 16-18" → "Psalm 80:1-7",
 *  "Psalm 80:16-18" (one scripture item each). */
export function readingParts(r: string): string[] {
  const parts = cleanReading(r).split(/,\s*/);
  const m = /^(.*?)\s(\d+):/.exec(parts[0]);
  if (!m || parts.length === 1) return [parts[0]];
  const [, book, chapter] = m;
  return parts.map((p, i) => (i === 0 ? p : /^\d+:/.test(p) ? `${book} ${p}` : `${book} ${chapter}:${p}`));
}

/** The next Sunday from today (today if it is one). */
function nextSunday(): string {
  const d = new Date();
  d.setDate(d.getDate() + ((7 - d.getDay()) % 7));
  return d.toISOString().slice(0, 10);
}

export function LectionaryPanel({ serviceDate, onAdd, onClose }: { serviceDate: string | null; onAdd: (items: ProgramItem[]) => void; onClose: () => void }) {
  const [date, setDate] = useState(serviceDate || nextSunday());
  const [l, setL] = useState<Lectionary | null>(null);
  const [bible, setBible] = useState<string>("");
  const [note, setNote] = useState<string | null>(null);
  useEffect(() => {
    api.lectionaryFor(date).then(setL).catch(() => setL(null));
  }, [date]);
  useEffect(() => {
    api.versions().then((v) => setBible(localStorage.getItem("wp-bible-primary") || v[0]?.code || "")).catch(() => {});
  }, []);
  const items = (r: string): ProgramItem[] => readingParts(r).map((reference) => ({ kind: "scripture", reference, versions: bible ? [bible] : [] }));
  const add = (rs: string[]) => {
    onAdd(rs.flatMap(items));
    setNote(rs.length === 1 ? `Added ${cleanReading(rs[0])}.` : `Added ${rs.length} readings to the service.`);
  };
  return (
    <aside className="opn lect" role="dialog" aria-label="Lectionary">
      <header className="opn__head">
        <h2>Lectionary</h2>
        <button className="wp-btn wp-btn--quiet" onClick={onClose}>
          Close
        </button>
      </header>
      <label className="ps__row">
        <span>Date</span>
        <input className="wp-input" type="date" value={date} onChange={(e) => e.target.value && setDate(e.target.value)} aria-label="Date" />
      </label>
      {l?.day ? (
        <div className="lect__day">
          <strong>{l.occasion || l.day.name}</strong>
          <span className="wp-note">
            {l.occasion && l.occasion !== l.day.name ? `${l.day.name} · ` : ""}Year {l.day.year} · {SEASONS[l.day.season] ?? l.day.season}
          </span>
        </div>
      ) : (
        l && <p className="wp-note">An ordinary weekday: no set readings.</p>
      )}
      {l && l.readings.length > 0 && (
        <>
          <ul className="lect__list">
            {l.readings.map((r, i) => (
              <li key={i}>
                <span>{r}</span>
                <button className="btn-quiet" onClick={() => add([r])}>
                  Add
                </button>
              </li>
            ))}
          </ul>
          <button className="wp-btn wp-btn--primary" onClick={() => add(l.readings)}>
            Add all to the service
          </button>
        </>
      )}
      {l && l.day && l.readings.length === 0 && (
        <p className="wp-note">
          {l.entries === 0 ? "Import your church's lectionary (Settings → Library → Lectionary) to see the readings here." : "Your lectionary has no readings for this day."}
        </p>
      )}
      {note && <p className="pkg__done">{note}</p>}
    </aside>
  );
}
