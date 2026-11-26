// Keyboard for lists (songs, media, themes, scripture results), as in any Windows list:
// Ctrl+A selects all, Delete deletes the selection (after asking), ↑ ↓ Home End move (with
// Shift: extend), Enter previews, Ctrl+Enter adds to the schedule. Keys a list handles stay in
// the list: they never reach the live keys (an arrow here does not change the slide). Esc is
// left alone: it blanks the screens, always.

import type React from "react";

export type ListState = {
  /** the ids in the order shown */
  ids: number[];
  sel: number | null;
  multi: Set<number>;
  setSel: (id: number | null) => void;
  setMulti: (s: Set<number>) => void;
  anchor: { current: number | null };
  onOpen?: (id: number) => void;
  onAdd?: (ids: number[]) => void;
  onDelete?: (ids: number[]) => void;
  /** keep a row in sight (a long, scrolling list) */
  reveal?: (index: number) => void;
};

/** The selection, in the list's order. */
export function selectedIds(s: Pick<ListState, "ids" | "sel" | "multi">): number[] {
  if (s.multi.size) return s.ids.filter((id) => s.multi.has(id));
  return s.sel != null && s.ids.includes(s.sel) ? [s.sel] : [];
}

/** What a key does to a list; true if it handled it. */
export function listKey(e: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey">, s: ListState): boolean {
  const ctrl = e.ctrlKey || e.metaKey;
  const n = s.ids.length;
  if (!n || e.altKey) return false;
  if (ctrl && e.key.toLowerCase() === "a") {
    s.setMulti(new Set(s.ids));
    return true;
  }
  const chosen = selectedIds(s);
  if (e.key === "Delete" || (e.key === "Backspace" && ctrl)) {
    if (!s.onDelete || !chosen.length) return false;
    s.onDelete(chosen);
    return true;
  }
  if (e.key === "Enter") {
    if (ctrl && s.onAdd && chosen.length) return s.onAdd(chosen), true;
    if (!ctrl && s.onOpen && chosen.length) return s.onOpen(chosen[0]), true;
    return false;
  }
  const at = (() => {
    const cur = s.sel != null ? s.ids.indexOf(s.sel) : -1;
    return cur >= 0 ? cur : chosen.length ? s.ids.indexOf(chosen[chosen.length - 1]) : -1;
  })();
  let to: number | null = null;
  if (e.key === "ArrowDown") to = Math.min(n - 1, at + 1);
  else if (e.key === "ArrowUp") to = Math.max(0, at < 0 ? 0 : at - 1);
  else if (e.key === "Home") to = 0;
  else if (e.key === "End") to = n - 1;
  if (to == null) return false;
  if (e.shiftKey) {
    const from = s.anchor.current ?? (at >= 0 ? at : to);
    s.anchor.current = from;
    const [a, b] = [Math.min(from, to), Math.max(from, to)];
    s.setMulti(new Set(s.ids.slice(a, b + 1)));
    s.setSel(s.ids[to]);
  } else {
    s.anchor.current = to;
    s.setMulti(new Set());
    s.setSel(s.ids[to]);
  }
  s.reveal?.(to);
  return true;
}

/** A list's onKeyDown: handled keys stop here (not the live keys, not the page). */
export function onListKey(s: ListState) {
  return (e: React.KeyboardEvent) => {
    const t = e.target as HTMLElement;
    if (t.closest("input, textarea, select, [contenteditable='true']")) return;
    if (listKey(e.nativeEvent, s)) {
      e.preventDefault();
      e.stopPropagation();
    }
  };
}
