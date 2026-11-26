// Languages: one list for the whole app, plus any the church types in (kept in Settings).
// Songs store a short code per language ("en", "fat"); names come from here.

import { useEffect, useState } from "react";
import { api, onSettingsChanged } from "./api";

export type Language = { code: string; name: string };

/** Ghana's languages first, then others churches here sing in. */
export const BUILT_IN: Language[] = [
  { code: "en", name: "English" },
  { code: "fat", name: "Fante" },
  { code: "tw", name: "Twi" },
  { code: "ak", name: "Akan" },
  { code: "gaa", name: "Ga" },
  { code: "ee", name: "Ewe" },
  { code: "dag", name: "Dagbani" },
  { code: "nzi", name: "Nzema" },
  { code: "ada", name: "Dangme" },
  { code: "gur", name: "Farefare" },
  { code: "kus", name: "Kusaal" },
  { code: "ha", name: "Hausa" },
  { code: "yo", name: "Yoruba" },
  { code: "ig", name: "Igbo" },
  { code: "sw", name: "Swahili" },
  { code: "zu", name: "Zulu" },
  { code: "ln", name: "Lingala" },
  { code: "fr", name: "French" },
  { code: "es", name: "Spanish" },
  { code: "pt", name: "Portuguese" },
  { code: "de", name: "German" },
  { code: "nl", name: "Dutch" },
  { code: "zh", name: "Chinese" },
  { code: "ar", name: "Arabic" },
];
const ALIASES: Record<string, string> = { ga: "Ga" };

let custom: Language[] = [];
let loaded = false;
const listeners = new Set<() => void>();
function load() {
  api
    .getSettings()
    .then((x) => {
      try {
        custom = x.languages_custom ? (JSON.parse(x.languages_custom) as Language[]) : [];
      } catch {
        custom = [];
      }
      listeners.forEach((l) => l());
    })
    .catch(() => {});
}

/** Every language: the built-in ones and the church's own. */
export function allLanguages(): Language[] {
  return [...BUILT_IN, ...custom.filter((c) => !BUILT_IN.some((b) => b.code === c.code))];
}

/** A language's name from its code (an unknown code is shown as it is). */
export function langName(code: string): string {
  return allLanguages().find((l) => l.code === code)?.name ?? ALIASES[code] ?? code;
}

/** Adds a language typed in ("Kasem"), kept for next time; returns its code. */
export async function addLanguage(name: string): Promise<string> {
  const n = name.trim();
  const known = allLanguages().find((l) => l.name.toLowerCase() === n.toLowerCase());
  if (known) return known.code;
  const base = n.toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "").slice(0, 12) || "lang";
  let code = base;
  for (let i = 2; allLanguages().some((l) => l.code === code); i++) code = `${base}${i}`;
  custom = [...custom, { code, name: n }];
  await api.setSetting("languages_custom", JSON.stringify(custom));
  listeners.forEach((l) => l());
  return code;
}

/** The language list, kept up to date in every view. */
export function useLanguages(): Language[] {
  const [, tick] = useState(0);
  useEffect(() => {
    const on = () => tick((x) => x + 1);
    listeners.add(on);
    if (!loaded) {
      loaded = true;
      load();
      void onSettingsChanged(load);
    }
    return () => void listeners.delete(on);
  }, []);
  return allLanguages();
}
