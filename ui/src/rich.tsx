import { Fragment, type ReactNode } from "react";

// Rich text in lyrics: **bold**, *italic*, __underline__ and {gold}colour{/} (a colour
// name or #rgb/#rrggbb). Shown on the screens; everywhere else the plain words.

export type Piece = { text: string; b?: boolean; i?: boolean; u?: boolean; color?: string };

const NAMES = "white|black|red|gold|yellow|orange|green|blue|purple|pink|grey|gray|silver";
const COLOUR = new RegExp(`^\\{(#[0-9a-fA-F]{3,8}|${NAMES})\\}$`, "i");
const TOKEN = new RegExp(`(\\*\\*|__|\\*|\\{\\/\\}|\\{(?:#[0-9a-fA-F]{3,8}|${NAMES})\\})`, "i");

export function parseRich(line: string): Piece[] {
  const out: Piece[] = [];
  const st = { b: false, i: false, u: false, color: undefined as string | undefined };
  for (const part of line.split(TOKEN)) {
    if (!part) continue;
    if (part === "**") st.b = !st.b;
    else if (part === "*") st.i = !st.i;
    else if (part === "__") st.u = !st.u;
    else if (part === "{/}") st.color = undefined;
    else if (COLOUR.test(part)) st.color = part.slice(1, -1);
    else out.push({ text: part, ...(st.b && { b: true }), ...(st.i && { i: true }), ...(st.u && { u: true }), ...(st.color && { color: st.color }) });
  }
  return out;
}

/** The words without the markers. */
export const stripRich = (line: string) => parseRich(line).map((p) => p.text).join("");

export const hasRich = (line: string) => TOKEN.test(line);

/** The line for the screens. */
export function renderRich(line: string): ReactNode {
  if (!hasRich(line)) return line;
  return parseRich(line).map((p, k) =>
    p.b || p.i || p.u || p.color ? (
      <span key={k} style={{ fontWeight: p.b ? 800 : undefined, fontStyle: p.i ? "italic" : undefined, textDecoration: p.u ? "underline" : undefined, color: p.color }}>
        {p.text}
      </span>
    ) : (
      <Fragment key={k}>{p.text}</Fragment>
    ),
  );
}

/** Responsive readings: "Leader:" / "L:" and "People:" / "P:" / "All:" at the start of a
 *  line. On the screens the prefix is hidden and the people's lines are bold. */
const LEADER = /^\s*(leader|minister|l|m)\s*:\s*/i;
const PEOPLE = /^\s*(people|all|congregation|p|c)\s*:\s*/i;
export function responsive(line: string): { who: "leader" | "people" | null; text: string } {
  if (PEOPLE.test(line)) return { who: "people", text: line.replace(PEOPLE, "") };
  if (LEADER.test(line)) return { who: "leader", text: line.replace(LEADER, "") };
  return { who: null, text: line };
}
/** A screen line: rich text, with a responsive reading's people's lines in bold. */
export function renderLine(line: string): ReactNode {
  const r = responsive(line);
  if (r.who === "people") return <strong className="tl-people">{renderRich(r.text)}</strong>;
  return renderRich(r.text);
}
