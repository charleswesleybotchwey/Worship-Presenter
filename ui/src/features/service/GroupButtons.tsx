import type { Slide } from "../../api";
import { groupColor } from "../../groups";
import { sectionName } from "./SlideList";

/** ProPresenter's group buttons: one per section of the song on screen; pressing one goes
 *  to the next time that section comes (the next chorus), else its first time. */
export function GroupButtons({ slides, current, onGo }: { slides: Slide[]; current: number; onGo: (i: number) => void }) {
  const labels: string[] = [];
  for (const s of slides) if (!s.image && !labels.includes(s.label)) labels.push(s.label);
  if (labels.length < 2) return null;
  const go = (label: string) => {
    // the start of each run of this section
    const starts = slides.map((s, i) => (s.label === label && (i === 0 || slides[i - 1].label !== label || slides[i - 1].occurrence !== s.occurrence) ? i : -1)).filter((i) => i >= 0);
    const next = starts.find((i) => i > current) ?? starts[0];
    if (next != null) onGo(next);
  };
  const here = slides[current]?.label;
  return (
    <div className="grp-buttons" role="toolbar" aria-label="Go to a section">
      {labels.map((l) => (
        <button key={l} className={`grp-btn ${l === here ? "is-here" : ""}`} style={{ "--grp": groupColor(l) } as React.CSSProperties} onClick={() => go(l)} title={`Go to ${sectionName(l)}`}>
          {l}
        </button>
      ))}
    </div>
  );
}
