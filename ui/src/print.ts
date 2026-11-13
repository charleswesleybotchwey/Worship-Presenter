import { api, type SongDoc } from "./api";
import { pieces } from "./chords";

// Printing through Windows' own print dialog (which can also save as PDF): the sheet is
// put in a print-only part of the page; everything else is hidden while printing.

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

export function printHtml(title: string, html: string) {
  let root = document.getElementById("print-root");
  if (!root) {
    root = document.createElement("div");
    root.id = "print-root";
    document.body.appendChild(root);
  }
  root.innerHTML = `<h1>${esc(title)}</h1>${html}`;
  const was = document.title;
  document.title = title; // the default file name when saving as PDF
  document.body.classList.add("is-printing");
  const done = () => {
    document.body.classList.remove("is-printing");
    document.title = was;
    root!.innerHTML = "";
    window.removeEventListener("afterprint", done);
  };
  window.addEventListener("afterprint", done);
  setTimeout(() => window.print(), 50);
}

const LANG: Record<string, string> = { en: "English", fat: "Fante", tw: "Twi", gaa: "Ga", ee: "Ewe" };

/** Sections of a lyrics body: "[V1]" lines start a section. */
function sections(body: string): { label: string; lines: string[] }[] {
  const out: { label: string; lines: string[] }[] = [];
  for (const raw of body.split("\n")) {
    const m = /^\[([^\]]+)\]\s*$/.exec(raw.trim());
    if (m) out.push({ label: m[1], lines: [] });
    else if (raw.trim()) (out[out.length - 1] ?? (out.push({ label: "", lines: [] }), out[0])).lines.push(raw.trim());
  }
  return out;
}

const NAMES: Record<string, string> = { V: "Verse", C: "Chorus", B: "Bridge", PC: "Pre-chorus", T: "Tag", E: "Ending", I: "Intro", O: "Outro" };
const sectionName = (l: string) => {
  const m = /^([A-Z]+)(\d*)$/.exec(l);
  return m && NAMES[m[1]] ? `${NAMES[m[1]]}${m[2] ? " " + m[2] : ""}` : l;
};
const plain = (line: string) => line.replace(/\[[^\]]*\]/g, "");

/** A song's words, every language side by side (chords left out). */
export async function printSongWords(id: number) {
  const s: SongDoc = await api.getSong(id);
  const cols = s.lyrics
    .map((l) => {
      const secs = sections(l.body)
        .map((x) => `<div class="pr-sec"><div class="pr-label">${esc(sectionName(x.label))}</div>${x.lines.map((t) => `<p>${esc(plain(t))}</p>`).join("")}</div>`)
        .join("");
      return `<div class="pr-col"><h2>${esc(LANG[l.lang] ?? l.lang)}</h2>${secs}</div>`;
    })
    .join("");
  printHtml(s.title, `${s.author ? `<p class="pr-meta">${esc(s.author)}</p>` : ""}<div class="pr-cols">${cols}</div>${s.copyright ? `<p class="pr-meta">${esc(s.copyright)}</p>` : ""}`);
}

/** A chord chart for the band: chords above the words, in the song's key. */
export async function printChordChart(id: number) {
  const s: SongDoc = await api.getSong(id);
  const first = s.lyrics[0];
  if (!first) return;
  const secs = sections(first.body)
    .map(
      (x) =>
        `<div class="pr-sec"><div class="pr-label">${esc(sectionName(x.label))}</div>${x.lines
          .map((line) => {
            const ps = pieces(line);
            if (!ps.some((p) => p.chord)) return `<p>${esc(line)}</p>`;
            return `<p class="pr-chordline">${ps.map((p) => `<span class="pr-piece"><b>${esc(p.chord || "\u00a0")}</b><span>${esc(p.text || "\u00a0")}</span></span>`).join("")}</p>`;
          })
          .join("")}</div>`,
    )
    .join("");
  printHtml(`${s.title}${s.key ? ` (key of ${s.key})` : ""}`, `<div class="pr-chart">${secs}</div>`);
}

/** The service order: each item in order. */
export function printServiceOrder(name: string, date: string | null, items: { title: string; kind: string }[]) {
  const kinds: Record<string, string> = { song: "Song", scripture: "Scripture", media: "Media", announcement: "Presentation" };
  printHtml(
    `${name}${date ? ` · ${date}` : ""}`,
    `<ol class="pr-order">${items.map((i) => `<li><span class="pr-kind">${esc(kinds[i.kind] ?? i.kind)}</span> ${esc(i.title)}</li>`).join("")}</ol>`,
  );
}

/** The CCLI song usage report as a table. */
export function printUsage(title: string, rows: { title: string; ccli?: string | null; count: number; author?: string | null }[]) {
  printHtml(
    title,
    `<table class="pr-table"><thead><tr><th>Song</th><th>Author</th><th>CCLI</th><th>Times</th></tr></thead><tbody>${rows
      .map((r) => `<tr><td>${esc(r.title)}</td><td>${esc(r.author ?? "")}</td><td>${esc(r.ccli ?? "")}</td><td>${r.count}</td></tr>`)
      .join("")}</tbody></table>`,
  );
}
