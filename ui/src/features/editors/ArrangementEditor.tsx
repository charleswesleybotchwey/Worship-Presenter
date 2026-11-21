import { useRef } from "react";
import { asWritten, chorusAfterEach, groupColor, parseOrder } from "../../groups";
import { sectionName } from "../service/SlideList";

/** ProPresenter's arrangement editor: the song's sections as coloured blocks, and the order
 *  they are sung in, built by clicking and dragging. Empty = as written. */
export function ArrangementEditor({ sections, order, onChange }: { sections: string[]; order: string; onChange: (order: string) => void }) {
  const list = parseOrder(order);
  const drag = useRef<number | null>(null);
  const set = (l: string[]) => onChange(l.join(" "));
  const chip = (label: string) => ({ "--grp": groupColor(label) }) as React.CSSProperties;
  return (
    <div className="arr">
      <div className="arr__row">
        <span className="arr__title">The song's sections</span>
        <div className="arr__chips">
          {sections.map((s) => (
            <button key={s} className="grp-btn" style={chip(s)} onClick={() => set([...list, s])} title={`Add ${sectionName(s)} to the order`}>
              + {s}
            </button>
          ))}
        </div>
      </div>
      <div className="arr__row">
        <span className="arr__title">Sung in this order</span>
        <div
          className="arr__chips arr__order"
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            // dropped past the last block: to the end
            if (drag.current == null || e.target !== e.currentTarget) return;
            const l = [...list];
            const [m] = l.splice(drag.current, 1);
            set([...l, m]);
            drag.current = null;
          }}
        >
          {list.length === 0 && <span className="wp-note">As written ({sections.join(" ")})</span>}
          {list.map((s, i) => (
            <span
              key={`${s}-${i}`}
              className={`grp-btn arr__item ${sections.includes(s) ? "" : "is-missing"}`}
              style={chip(s)}
              draggable
              onDragStart={() => (drag.current = i)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                e.stopPropagation();
                if (drag.current == null || drag.current === i) return;
                const l = [...list];
                const [m] = l.splice(drag.current, 1);
                l.splice(i, 0, m);
                set(l);
                drag.current = null;
              }}
              title={sections.includes(s) ? `${sectionName(s)}: drag to move` : `${s} is not in the words`}
            >
              {s}
              <button className="arr__x" aria-label={`Remove ${s}`} onClick={() => set(list.filter((_, k) => k !== i))}>
                ×
              </button>
            </span>
          ))}
        </div>
      </div>
      <div className="arr__row">
        <span className="arr__title" />
        <div className="arr__chips">
          <button className="btn-quiet" onClick={() => set(asWritten(sections))}>
            As written
          </button>
          <button className="btn-quiet" onClick={() => set(chorusAfterEach(sections))}>
            Chorus after each verse
          </button>
          <button className="btn-quiet" onClick={() => onChange("")}>
            Clear
          </button>
        </div>
      </div>
    </div>
  );
}
