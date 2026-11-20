// The song editor's Tidy tools (EasyWorship's reformatting). Section marks ("[V1]") are
// left alone; only the words change.

const isMark = (l: string) => /^\s*\[[^\]]+\]\s*$/.test(l);
const eachLine = (body: string, f: (l: string) => string) =>
  body
    .split("\n")
    .map((l) => (isMark(l) || !l.trim() ? l : f(l)))
    .join("\n");

/** "o for a thousand tongues" → "O for a thousand tongues" (Ghanaian letters too). */
export const capitaliseLines = (body: string) => eachLine(body, (l) => l.replace(/^(\s*)(\S)/u, (_, sp: string, c: string) => sp + c.toLocaleUpperCase()));

/** Removes , ; : . and dashes at the ends of lines (keeps ! ? and closing quotes). */
export const trimLineEnds = (body: string) => eachLine(body, (l) => l.replace(/[\s,;:.\-–—]+$/u, ""));

/** Single spaces, none at the ends of lines. */
export const tidySpaces = (body: string) => eachLine(body, (l) => l.replace(/[ \t\u00a0]+/g, " ").trim());

/** No empty lines inside a section (one blank line before each section mark). */
export function dropBlankLines(body: string): string {
  const out: string[] = [];
  for (const l of body.split("\n")) {
    if (!l.trim()) continue;
    if (isMark(l) && out.length) out.push("");
    out.push(l);
  }
  return out.join("\n");
}
