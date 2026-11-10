import { useEffect, useRef, useState } from "react";
import { api, onLive, type Broadcast, type OutputStatus } from "../api";

// What each output has actually drawn. "Live" means the renderer acknowledged the
// revision on air (design brief §6); a request without an acknowledgement shows as
// Sending…, then Not confirmed after 1 s; an output silent for 3 s is Disconnected.

export type OutputCondition = "none" | "confirmed" | "sending" | "unconfirmed" | "disconnected";

export interface OutputView {
  role: string;
  condition: OutputCondition;
  /** The broadcast this output last confirmed, for the "last confirmed" monitor. */
  confirmed: Broadcast | null;
  confirmedAt: number | null;
}

export const ROLE_NAMES: Record<string, string> = { main: "Main", alternate: "Lower third", foldback: "Stage" };

export function useLive() {
  const [live, setLive] = useState<Broadcast | null>(null);
  const [status, setStatus] = useState<OutputStatus[]>([]);
  const history = useRef<Broadcast[]>([]);
  const requestedAt = useRef<number>(0);
  const confirmedAt = useRef<Record<string, { rev: number; at: number }>>({});

  useEffect(() => {
    let stop = () => {};
    onLive((b) => {
      const last = history.current[history.current.length - 1];
      if (!last || b.state.revision !== last.state.revision) requestedAt.current = Date.now();
      history.current = [...history.current.slice(-40), b];
      setLive(b);
    }).then((u) => (stop = u));
    api.resync().catch(() => {}); // a recreated control window asks for a fresh snapshot
    const poll = setInterval(() => api.outputStatus().then(setStatus).catch(() => {}), 400);
    return () => (stop(), clearInterval(poll));
  }, []);

  const view = (role: string): OutputView => {
    const s = status.find((x) => x.role === role);
    if (!s) return { role, condition: "none", confirmed: null, confirmedAt: null };
    const want = live?.state.revision ?? 0;
    const prev = confirmedAt.current[role];
    if (!prev || prev.rev !== s.revision) confirmedAt.current[role] = { rev: s.revision, at: Date.now() };
    // the newest broadcast at or below what the output acknowledged
    const confirmed = [...history.current].reverse().find((b) => b.state.revision <= s.revision) ?? null;
    let condition: OutputCondition;
    if (s.silent_secs > 3) condition = "disconnected";
    else if (s.revision >= want) condition = "confirmed";
    else condition = Date.now() - requestedAt.current > 1000 ? "unconfirmed" : "sending";
    return { role, condition, confirmed, confirmedAt: confirmedAt.current[role]?.at ?? null };
  };

  return { live, roles: status.map((s) => s.role), view };
}

/** "Live · Main", "Main: blank", "Main: not confirmed" … */
export function outputLabel(v: OutputView, mode: string | undefined): string {
  const name = ROLE_NAMES[v.role] ?? v.role;
  switch (v.condition) {
    case "none":
      return v.role === "main" ? "No main display" : `${name}: off`;
    case "disconnected":
      return `${name}: disconnected`;
    case "unconfirmed":
      return `${name}: not confirmed`;
    case "sending":
      return `${name}: sending…`;
    case "confirmed": {
      const m = v.confirmed?.state.mode;
      if (!v.confirmed?.item) return `${name}: ready`;
      return m === "blank" ? `${name}: blank` : m === "logo" ? `${name}: logo` : m === "clear" ? `${name}: text cleared` : `Live · ${name}`;
    }
  }
  return mode ?? "";
}
