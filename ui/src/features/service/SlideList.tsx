import { stripRich } from "../../rich";
import { groupColor } from "../../groups";
import { useEffect, useRef } from "react";
import type { Slide } from "../../api";
import { Icon } from "../../Icon";

// EasyWorship-style slide list: a header for each section ("Verse 3"), numbered
// slides beneath with their lines (second language below in its own colour).

const NAMES: Record<string, string> = { V: "Verse", C: "Chorus", B: "Bridge", PC: "Pre-chorus", T: "Tag", E: "Ending", I: "Intro", O: "Outro" };

export function sectionName(label: string) {
  const m = label.match(/^([A-Z]+)(\d*)$/);
  if (!m || !NAMES[m[1]]) return label;
  return `${NAMES[m[1]]}${m[2] ? " " + m[2] : ""}`;
}

export function SlideList({
  slides,
  current,
  onPick,
  onActivate,
  live,
  confirmed = true,
  empty,
  marked,
  onMenu,
}: {
  /** slides with a slide action (⚡) */
  marked?: (i: number) => boolean;
  /** right-click on a slide */
  onMenu?: (i: number, x: number, y: number) => void;
  slides: Slide[];
  current: number;
  onPick: (i: number) => void;
  onActivate?: (i: number) => void;
  live?: boolean;
  confirmed?: boolean;
  empty: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>(".sl-slide.is-current")?.scrollIntoView({ block: "nearest" });
  }, [current]);
  if (!slides.length) return <p className="wp-empty wp-empty--inline">{empty}</p>;
  const groups: { head: string; items: { s: Slide; i: number }[] }[] = [];
  slides.forEach((s, i) => {
    const head = `${s.label}#${s.occurrence}`;
    if (!groups.length || groups[groups.length - 1].head !== head) groups.push({ head, items: [] });
    groups[groups.length - 1].items.push({ s, i });
  });
  return (
    <div className="sl" ref={ref} role="listbox" aria-label={live ? "Live slides: choosing one changes the screens" : "Preview slides"}>
      {groups.map((g) => {
        const on = g.items.some((x) => x.i === current);
        return (
          <section key={g.head} className={`sl-group ${on ? "is-current" : ""}`} style={{ "--grp": groupColor(g.items[0].s.label) } as React.CSSProperties}>
            <div className="sl-head">{sectionName(g.items[0].s.label)}</div>
            {g.items.map(({ s, i }) => (
              <div
                key={i}
                role="option"
                aria-selected={i === current}
                className={`sl-slide ${i === current ? "is-current" : ""}`}
                onClick={() => onPick(i)}
                onDoubleClick={onActivate ? () => onActivate(i) : undefined}
                onContextMenu={onMenu ? (e) => (e.preventDefault(), onMenu(i, e.clientX, e.clientY)) : undefined}
                title={live ? "Show this slide on the screens" : onActivate ? "Click to stage, double-click to go live" : undefined}
              >
                <span className="sl-num wp-num">
                  {i + 1}
                  {marked?.(i) && (
                    <span className="sl-action" title="This slide runs a macro when it goes live">
                      ⚡
                    </span>
                  )}
                </span>
                <span className="sl-text">
                  {s.image ? <em>Picture</em> : s.primary.map((l, k) => <span key={k}>{stripRich(l)}</span>)}
                  {s.second.map((l, k) => (
                    <span key={`s${k}`} className="sl-second">
                      {l}
                    </span>
                  ))}
                </span>
                {live && i === current && confirmed && (
                  <span className="wp-live-tag">
                    <Icon name="speaker" /> Live
                  </span>
                )}
              </div>
            ))}
          </section>
        );
      })}
    </div>
  );
}
