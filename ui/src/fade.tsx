import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { api, onSettingsChanged } from "./api";

// Smoother video (as ProPresenter): a crossfade when the picture changes — the outgoing
// video stays mounted (so it keeps playing, never restarts) and fades out while the new one
// fades in. For that moment two videos run at once.

let fadeMs = 700;
const listeners = new Set<(ms: number) => void>();
let loaded = false;
function loadFade() {
  api
    .getSettings()
    .then((x) => {
      const v = Number(x.media_fade_ms);
      fadeMs = Number.isFinite(v) && x.media_fade_ms !== undefined && x.media_fade_ms !== "" ? Math.max(0, Math.min(3000, v)) : 700;
      listeners.forEach((l) => l(fadeMs));
    })
    .catch(() => {});
}

/** The crossfade length (Settings; 0 = cut), shared by every view. */
export function useMediaFadeMs(): number {
  const [ms, setMs] = useState(fadeMs);
  useEffect(() => {
    listeners.add(setMs);
    if (!loaded) {
      loaded = true;
      loadFade();
      void onSettingsChanged(loadFade);
    }
    return () => void listeners.delete(setMs);
  }, []);
  return ms;
}

/** Crossfades when `k` changes. */
export function Crossfade({ k, ms, children, className = "" }: { k: string; ms: number; children: ReactNode; className?: string }) {
  const last = useRef<{ k: string; node: ReactNode }>({ k, node: children });
  const [prev, setPrev] = useState<{ k: string; node: ReactNode } | null>(null);
  // the content going out: the one shown until now, the moment the key changes
  const outgoing = last.current.k !== k ? last.current : prev;
  if (last.current.k === k) last.current.node = children;
  useEffect(() => {
    if (last.current.k === k) return;
    const old = last.current;
    last.current = { k, node: children };
    if (ms <= 0) return void setPrev(null);
    setPrev(old);
    const t = setTimeout(() => setPrev(null), ms);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [k]);
  const dur = { animationDuration: `${ms}ms` } as CSSProperties;
  const layers = [
    ...(outgoing && ms > 0 && outgoing.k !== k ? [{ key: outgoing.k, node: outgoing.node, cls: "xf__out" }] : []),
    { key: k, node: children, cls: outgoing && ms > 0 ? "xf__in" : "" },
  ];
  return (
    <div className={`xf ${className}`}>
      {layers.map((l) => (
        <div key={l.key} className={`xf__layer ${l.cls}`} style={l.cls ? dur : undefined}>
          {l.node}
        </div>
      ))}
    </div>
  );
}
