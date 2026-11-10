import { useEffect, useState } from "react";

// Operator preferences kept per computer: keyboard map, single-letter shortcuts,
// low-spec interface. Shared by every window (same storage), never by the outputs'
// projection logic except low-spec, which the core setting governs.

export type Action = "next" | "previous" | "nextItem" | "previousItem" | "blank" | "logo" | "clear";

export const ACTION_NAMES: Record<Action, string> = {
  next: "Next slide",
  previous: "Previous slide",
  nextItem: "Next service item",
  previousItem: "Previous service item",
  blank: "Blank",
  logo: "Logo",
  clear: "Clear text",
};

export const DEFAULT_KEYS: Record<Action, string[]> = {
  next: [" ", "Enter", "ArrowRight"],
  previous: ["ArrowLeft"],
  nextItem: ["PageDown"],
  previousItem: ["PageUp"],
  blank: ["Escape"],
  logo: ["l"],
  clear: ["c"],
};

/** Keys the app keeps for itself everywhere. */
export const RESERVED = ["F5", "F6", "Tab", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

export interface Prefs {
  keys: Record<Action, string[]>;
  singleLetter: boolean;
  lowSpecUi: boolean;
}

const DEFAULTS: Prefs = { keys: DEFAULT_KEYS, singleLetter: true, lowSpecUi: false };

export function loadPrefs(): Prefs {
  try {
    const p = JSON.parse(localStorage.getItem("wp-prefs") ?? "{}");
    return { ...DEFAULTS, ...p, keys: { ...DEFAULT_KEYS, ...(p.keys ?? {}) } };
  } catch {
    return DEFAULTS;
  }
}

export function savePrefs(p: Prefs) {
  localStorage.setItem("wp-prefs", JSON.stringify(p));
  window.dispatchEvent(new Event("wp-prefs"));
}

export function usePrefs(): [Prefs, (p: Prefs) => void] {
  const [p, setP] = useState(loadPrefs);
  useEffect(() => {
    const f = () => setP(loadPrefs());
    window.addEventListener("wp-prefs", f);
    window.addEventListener("storage", f);
    return () => (window.removeEventListener("wp-prefs", f), window.removeEventListener("storage", f));
  }, []);
  return [p, savePrefs];
}

export const isSingleLetter = (k: string) => k.length === 1 && /\p{L}/u.test(k);

/** The action a key triggers on the Live surface, honouring the single-letter switch. */
export function actionFor(p: Prefs, key: string): Action | null {
  const k = key.length === 1 ? key.toLowerCase() : key;
  for (const a of Object.keys(p.keys) as Action[]) {
    if (p.keys[a].some((x) => (x.length === 1 ? x.toLowerCase() : x) === k)) {
      if (!p.singleLetter && isSingleLetter(k)) return null;
      return a;
    }
  }
  return null;
}

/** Which action already uses a key, for conflict warnings when rebinding. */
export function conflict(p: Prefs, key: string, except: Action): Action | null {
  const k = key.length === 1 ? key.toLowerCase() : key;
  return (Object.keys(p.keys) as Action[]).find((a) => a !== except && p.keys[a].some((x) => (x.length === 1 ? x.toLowerCase() : x) === k)) ?? null;
}

export const keyName = (k: string) => (k === " " ? "Space" : k === "Escape" ? "Esc" : k.startsWith("Arrow") ? `${k.slice(5)} arrow` : k.length === 1 ? k.toUpperCase() : k);
