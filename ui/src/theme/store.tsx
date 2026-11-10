import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api, onLive, onSettingsChanged } from "../api";
import { BUILTIN, setFontFallback, type ActiveThemes, type Theme } from "./types";

// The default theme of each kind, shared by every window. Refetched only when the
// core's theme revision changes (themes can hold images, so they are not sent
// with every slide change).

const Ctx = createContext<ActiveThemes>({ rev: 0, ...BUILTIN });

export function ThemesProvider({ children }: { children: ReactNode }) {
  const [themes, setThemes] = useState<ActiveThemes>({ rev: 0, ...BUILTIN });
  const rev = useRef(-1);
  useEffect(() => {
    const load = () => api.activeThemes().then((t) => ((rev.current = t.rev), setThemes(t))).catch(() => {});
    load();
    let stop = () => {};
    onLive((b) => b.theme_rev !== rev.current && load()).then((u) => (stop = u));
    // font fallback list from Settings; redraw everything when it changes
    const fonts = () =>
      api
        .getSettings()
        .then((x) => {
          setFontFallback((x.font_fallback ?? "").split(","));
          setThemes((t) => ({ ...t }));
        })
        .catch(() => {});
    fonts();
    let stop2 = () => {};
    onSettingsChanged(fonts).then((u) => (stop2 = u));
    return () => (stop(), stop2());
  }, []);
  return <Ctx.Provider value={themes}>{children}</Ctx.Provider>;
}

export const useThemes = () => useContext(Ctx);

// Per-item looks: fetched by id and cached until any theme changes.
const cache = new Map<string, Promise<Theme | null>>();

export function useTheme(kind: "song" | "scripture" | "lower_third" | "announcement", themeId?: number | null): Theme {
  const themes = useThemes();
  const base = themes[kind];
  const [override, setOverride] = useState<Theme | null>(null);
  useEffect(() => {
    if (themeId == null || kind === "lower_third" || kind === "announcement") return setOverride(null);
    const key = `${themeId}@${themes.rev}`;
    if (!cache.has(key)) cache.set(key, api.getTheme(themeId).catch(() => null));
    let alive = true;
    cache.get(key)!.then((t) => alive && setOverride(t));
    return () => {
      alive = false;
    };
  }, [kind, themeId, themes.rev]);
  return override ?? base;
}
