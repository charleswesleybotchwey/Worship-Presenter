import { useEffect, useState } from "react";
import { api } from "./api";

// The output dashboard: every screen, NDI stream and the recording at a glance, and
// "Check system" before the service (each problem with where to fix it).

export type Tile = { role: string; name: string; kind: string; state: string; detail: string };
export type Check = { id: string; label: string; status: "ok" | "warn" | "bad"; detail: string; fix: string | null };

export const STATE_NAMES: Record<string, string> = {
  live: "Live",
  idle: "Ready (nothing live)",
  blank: "Blank",
  logo: "Logo",
  clear: "Words hidden",
  held: "Held",
  silent: "Not answering",
  off: "Off",
  recording: "Recording",
  streaming: "Live online",
};

/** The dashboard, refreshed every 2 seconds while shown. */
export function useDashboard(): { tiles: Tile[]; screens_off: boolean } | null {
  const [d, setD] = useState<{ tiles: Tile[]; screens_off: boolean } | null>(null);
  useEffect(() => {
    let on = true;
    const load = () => api.dashboard().then((x) => on && setD(x)).catch(() => {});
    load();
    const t = setInterval(load, 2000);
    return () => ((on = false), clearInterval(t));
  }, []);
  return d;
}

/** The small dots in the service bar: one per output, coloured by its state. */
export function OutputDots({ onOpen }: { onOpen: () => void }) {
  const d = useDashboard();
  const tiles = d?.tiles ?? [];
  const worst = tiles.some((t) => t.state === "off" || t.state === "silent") ? "bad" : tiles.some((t) => t.state === "held") ? "warn" : "ok";
  return (
    <button className={`odots odots--${worst}`} onClick={onOpen} title={tiles.length ? tiles.map((t) => `${t.name}: ${STATE_NAMES[t.state] ?? t.state}`).join("\n") : "No outputs set up"} aria-label="Outputs">
      {tiles.length ? tiles.map((t) => <span key={t.role} className={`odot odot--${t.state}`} />) : <span className="odot odot--off" />}
    </button>
  );
}

export function OutputsPanel({ onClose, onFix }: { onClose: () => void; onFix: (where: string) => void }) {
  const d = useDashboard();
  const [checks, setChecks] = useState<Check[] | null>(null);
  const [busy, setBusy] = useState(false);
  const runCheck = async () => {
    setBusy(true);
    try {
      setChecks(await api.systemCheck());
    } finally {
      setBusy(false);
    }
  };
  const bad = checks?.filter((c) => c.status === "bad").length ?? 0;
  const warn = checks?.filter((c) => c.status === "warn").length ?? 0;
  return (
    <aside className="opn" role="dialog" aria-label="Outputs">
      <header className="opn__head">
        <h2>Outputs</h2>
        <button className="wp-btn wp-btn--quiet" onClick={onClose}>
          Close
        </button>
      </header>
      <div className="opn__tiles">
        {d?.tiles.length === 0 && <p className="wp-note">No outputs set up yet.</p>}
        {d?.tiles.map((t) => (
          <div key={t.role} className={`opn__tile opn__tile--${t.state}`}>
            <span className={`odot odot--${t.state}`} />
            <div>
              <strong>{t.name}</strong>
              <span className="opn__kind">{t.kind === "ndi" ? "NDI" : t.kind === "recording" ? "" : "Monitor"}</span>
              <div className="opn__state">{STATE_NAMES[t.state] ?? t.state}</div>
              <div className="wp-note">{t.detail}</div>
            </div>
          </div>
        ))}
      </div>
      <div className="opn__check">
        <button className="wp-btn wp-btn--primary" disabled={busy} onClick={() => void runCheck()}>
          {busy ? "Checking…" : "Check system"}
        </button>
        {checks && (
          <p className={`opn__sum opn__sum--${bad ? "bad" : warn ? "warn" : "ok"}`}>
            {bad ? `${bad} problem${bad > 1 ? "s" : ""} to fix before the service` : warn ? `Ready, with ${warn} thing${warn > 1 ? "s" : ""} to look at` : "Everything is ready."}
          </p>
        )}
        <ul className="opn__list">
          {checks?.map((c) => (
            <li key={c.id} className={`opn__item opn__item--${c.status}`}>
              <span className="opn__mark" aria-label={c.status}>
                {c.status === "ok" ? "✓" : c.status === "warn" ? "!" : "✕"}
              </span>
              <div>
                <strong>{c.label}</strong>
                <div className="wp-note">{c.detail}</div>
              </div>
              {c.fix && (
                <button className="wp-btn wp-btn--quiet" onClick={() => onFix(c.fix!)}>
                  Fix
                </button>
              )}
            </li>
          ))}
        </ul>
      </div>
    </aside>
  );
}
