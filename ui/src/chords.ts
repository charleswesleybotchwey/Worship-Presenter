// Inline chords ("[G]Amazing [C]grace") for the musicians' stage view: split a line
// into chord-over-words pieces, and move chords up or down by semitones.

const SHARPS = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
const FLATS = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
const ROOT = /^([A-G])([#b]?)/;

export function isChord(s: string) {
  return /^[A-G][#b]?(maj|min|sus|dim|aug|add|m|M|\+|°|ø)?[0-9#b()+\-susaddmajdimaug]*(\/[A-G][#b]?)?$/.test(s);
}

function moveNote(note: string, n: number, flats: boolean) {
  const m = note.match(ROOT);
  if (!m) return note;
  const base = SHARPS.indexOf(m[1]) + (m[2] === "#" ? 1 : m[2] === "b" ? -1 : 0);
  const i = (((base + n) % 12) + 12) % 12;
  return (flats ? FLATS : SHARPS)[i] + note.slice(m[0].length);
}

/** "Bbm7/F" up 2 → "Cm7/G". Flat keys keep flat names. */
export function transpose(chord: string, n: number, flats = false) {
  if (!n) return chord;
  const [main, bass] = chord.split("/");
  return moveNote(main, n, flats) + (bass ? "/" + moveNote(bass, n, flats) : "");
}

/** Whether a key reads better with flats (F, Bb, Eb, Ab, Db, Gb and minor keys like Dm). */
export function prefersFlats(key: string | null | undefined, n: number) {
  if (!key) return n < 0;
  const k = transpose(key.replace(/m$/, ""), n);
  return ["F", "A#", "D#", "G#", "C#", "Bb", "Eb", "Ab", "Db", "Gb"].includes(k) || /b/.test(k);
}

/** Pieces of a chord line: the chord (if any) and the words under it. */
export function pieces(line: string): { chord: string; text: string }[] {
  const out: { chord: string; text: string }[] = [];
  const re = /\[([^\]]+)\]/g;
  let last = 0;
  let pending = "";
  let m: RegExpExecArray | null;
  while ((m = re.exec(line))) {
    if (!isChord(m[1].trim())) continue;
    const text = line.slice(last, m.index);
    if (text || pending) out.push({ chord: pending, text });
    pending = m[1].trim();
    last = m.index + m[0].length;
  }
  out.push({ chord: pending, text: line.slice(last) });
  return out.filter((p) => p.chord || p.text);
}
