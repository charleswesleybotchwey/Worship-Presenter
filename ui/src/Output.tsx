import { useEffect, useRef, useState } from "react";
import { api, onHeartbeat, onLive, type Broadcast } from "./api";
import { SlideView } from "./SlideView";

// An output window. It draws only what the core sends, ignores anything older than
// what it already shows, and switches to its fallback picture (theme background,
// never black desktop) when the heartbeat stops for three seconds.

const RULES: Record<string, { onLogo: "logo" | "blank" | "transparent"; onBlank: "blank" | "transparent"; lowerThird: boolean; keyed: boolean }> = {
  main: { onLogo: "logo", onBlank: "blank", lowerThird: false, keyed: false },
  alternate: { onLogo: "transparent", onBlank: "transparent", lowerThird: true, keyed: true },
  foldback: { onLogo: "blank", onBlank: "blank", lowerThird: false, keyed: false },
};

export function Output({ role }: { role: string }) {
  const rule = RULES[role] ?? RULES.main;
  const [b, setB] = useState<Broadcast | null>(null);
  const [lost, setLost] = useState(false);
  const shown = useRef(0);
  const beat = useRef(Date.now());

  useEffect(() => {
    document.body.classList.add("output-body", `output-body--${role}`);
    let stop1 = () => {}, stop2 = () => {};
    onLive((x) => {
      if (x.state.revision < shown.current) return; // stale
      shown.current = x.state.revision;
      beat.current = Date.now();
      setLost(false);
      setB(x);
    }).then((u) => (stop1 = u));
    onHeartbeat(() => {
      beat.current = Date.now();
      setLost(false);
    }).then((u) => (stop2 = u));
    api.resync().catch(() => {});
    const t = setInterval(() => setLost(Date.now() - beat.current > 3000), 500);
    return () => {
      stop1();
      stop2();
      clearInterval(t);
    };
  }, [role]);

  const mode = b?.state.mode ?? "blank";
  const slide = b?.slides[b.state.slide_idx];
  const showLogo = mode === "logo" && rule.onLogo === "logo";
  const hideText = mode !== "live" || lost;
  const frameKey = `${b?.state.item_idx}-${b?.state.slide_idx}-${b?.state.lines_per_slide}-${b?.state.lang_mode}-${b?.slides.length}`;

  return (
    <div className={`output ${rule.keyed ? "output--keyed" : ""} ${mode === "blank" && !rule.keyed ? "output--black" : ""}`}>
      {showLogo && !lost && <div className="output__logo" aria-label="Logo" />}
      <div className="output__frame" key={frameKey}>
        <SlideView slide={slide} caption={b?.item?.kind === "scripture" ? b.caption : null} blank={hideText} look={{ lowerThird: rule.lowerThird, transparent: rule.keyed }} />
      </div>
    </div>
  );
}
