// Slide groups (as ProPresenter): colours by kind, the sections of a song, and
// arrangements (the order the sections are sung in).

const COLOURS: Record<string, string> = {
  V: "#6c9bd2", // verse: steel blue
  C: "#d9705f", // chorus: coral
  PC: "#cf9f45", // pre-chorus: amber
  B: "#9a86cf", // bridge: purple
  T: "#5aae9f", // tag: teal
  I: "#7fae6b", // intro: green
  O: "#7fae6b", // outro: green
  E: "#8e8a83", // ending: grey
};

/** "V2" → blue, "C" → red… (other labels: slate). */
export function groupColor(label: string): string {
  const m = /^([A-Z]+)\d*$/.exec(label.trim());
  return (m && COLOURS[m[1]]) || "#8e8a83";
}

/** The sections of a lyrics body, in order, each once: "[V1]…[C]…[V2]" → V1 C V2. */
export function sectionsOf(body: string): string[] {
  const out: string[] = [];
  for (const line of body.split("\n")) {
    const m = /^\s*\[([^\]]+)\]\s*$/.exec(line);
    if (m && !out.includes(m[1].trim())) out.push(m[1].trim());
  }
  return out;
}

/** The arrangement as written: each section once, in order. */
export const asWritten = (sections: string[]) => [...sections];

/** The chorus after every verse (and the bridge's chorus too): V1 C V2 C B C. */
export function chorusAfterEach(sections: string[]): string[] {
  const chorus = sections.find((s) => /^C\d*$/.test(s));
  if (!chorus) return [...sections];
  const out: string[] = [];
  for (const s of sections) {
    if (s === chorus) continue;
    out.push(s);
    if (/^(V|B)\d*$/.test(s)) out.push(chorus);
  }
  if (!out.includes(chorus)) out.push(chorus);
  return out;
}

export const parseOrder = (seq: string) => seq.split(/\s+/).map((s) => s.trim()).filter(Boolean);
