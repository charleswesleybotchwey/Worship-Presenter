import { useEffect, useMemo, useRef, useState } from "react";

// Ctrl+K command palette (design brief §5). Runs only safe actions: anything that
// would change public output opens its control instead of firing from search.

export interface PaletteCommand {
  id: string;
  group: string;
  label: string;
  hint?: string;
  keywords?: string;
  run: () => void;
}

export function Palette({ commands, onClose }: { commands: PaletteCommand[]; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const opener = useRef<Element | null>(document.activeElement);

  const shown = useMemo(() => {
    const t = q.trim().toLowerCase();
    return commands.filter((c) => !t || `${c.label} ${c.group} ${c.keywords ?? ""}`.toLowerCase().includes(t));
  }, [q, commands]);

  useEffect(() => {
    input.current?.focus();
    return () => (opener.current as HTMLElement | null)?.focus?.();
  }, []);
  useEffect(() => setSel(0), [q]);

  const close = () => onClose();
  const exec = (c: PaletteCommand | undefined) => {
    if (!c) return;
    onClose();
    // run after the palette has gone so focus lands where the command puts it
    setTimeout(c.run, 0);
  };

  let lastGroup = "";
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="palette" role="dialog" aria-modal="true" aria-label="Commands">
        <input
          ref={input}
          className="palette__input"
          value={q}
          placeholder="Type a command"
          aria-label="Command"
          aria-activedescendant={shown[sel] ? `cmd-${shown[sel].id}` : undefined}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") e.preventDefault(), setSel((i) => Math.min(shown.length - 1, i + 1));
            else if (e.key === "ArrowUp") e.preventDefault(), setSel((i) => Math.max(0, i - 1));
            else if (e.key === "Enter") e.preventDefault(), exec(shown[sel]);
            else if (e.key === "Escape") e.preventDefault(), e.stopPropagation(), close();
          }}
        />
        <ul className="palette__list" role="listbox" aria-label="Commands">
          {shown.map((c, i) => {
            const head = c.group !== lastGroup ? c.group : null;
            lastGroup = c.group;
            return (
              <li key={c.id} role="presentation">
                {head && <div className="palette__group">{head}</div>}
                <div
                  id={`cmd-${c.id}`}
                  role="option"
                  aria-selected={i === sel}
                  className={`palette__row ${i === sel ? "is-selected" : ""}`}
                  onMouseMove={() => setSel(i)}
                  onClick={() => exec(c)}
                >
                  <span>{c.label}</span>
                  {c.hint && <kbd>{c.hint}</kbd>}
                </div>
              </li>
            );
          })}
          {!shown.length && <li className="palette__empty">No command matches “{q}”.</li>}
        </ul>
      </div>
    </div>
  );
}
