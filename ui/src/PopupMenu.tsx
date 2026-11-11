import { useEffect, useLayoutEffect, useRef, useState } from "react";

// A Windows-style right-click menu: submenus (open on hover or →), check marks,
// greyed-out items, separators and access keys ("&Delete" underlines D; pressing D
// chooses it). Esc or ← closes a submenu; Esc on the top level closes the menu.

export type MenuEntry =
  | "-"
  | {
      label: string;
      run?: () => void;
      disabled?: boolean;
      checked?: boolean;
      sub?: MenuEntry[];
    };

function Label({ text }: { text: string }) {
  const i = text.indexOf("&");
  if (i < 0 || i === text.length - 1) return <>{text}</>;
  return (
    <>
      {text.slice(0, i)}
      <u>{text[i + 1]}</u>
      {text.slice(i + 2)}
    </>
  );
}

const accessKey = (t: string) => {
  const i = t.indexOf("&");
  return i >= 0 && i < t.length - 1 ? t[i + 1].toLowerCase() : null;
};

function Level({
  entries,
  x,
  y,
  flipFrom,
  depth,
  onDone,
  onBack,
}: {
  entries: MenuEntry[];
  x: number;
  y: number;
  /** For a submenu: the parent item's left edge, to open leftwards if there is no room. */
  flipFrom?: number;
  depth: number;
  onDone: () => void;
  onBack?: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y, ready: false });
  const [open, setOpen] = useState<{ i: number; x: number; y: number; left: number } | null>(null);
  const hoverTimer = useRef<number>();

  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (!r) return;
    let nx = x;
    if (nx + r.width > window.innerWidth - 4) nx = flipFrom != null ? Math.max(4, flipFrom - r.width + 2) : Math.max(4, window.innerWidth - r.width - 4);
    const ny = Math.max(4, Math.min(y, window.innerHeight - r.height - 4));
    setPos({ x: nx, y: ny, ready: true });
  }, [x, y, flipFrom]);

  useEffect(() => {
    if (pos.ready && depth === 0) ref.current?.querySelector<HTMLElement>("button:not([disabled])")?.focus();
  }, [pos.ready, depth]);

  const items = () => [...(ref.current?.querySelectorAll<HTMLButtonElement>(":scope > button:not([disabled])") ?? [])];
  const openSub = (i: number, el: HTMLElement, focus: boolean) => {
    const r = el.getBoundingClientRect();
    setOpen({ i, x: r.right - 2, y: r.top - 4, left: r.left });
    if (focus) setTimeout(() => document.querySelector<HTMLElement>(`[data-menu-depth="${depth + 1}"] button:not([disabled])`)?.focus(), 0);
  };

  return (
    <>
      <div
        ref={ref}
        className="pm"
        role="menu"
        data-menu-depth={depth}
        style={{ left: pos.x, top: pos.y, visibility: pos.ready ? "visible" : "hidden" }}
        onKeyDown={(e) => {
          const list = items();
          const at = list.indexOf(document.activeElement as HTMLButtonElement);
          const key = e.key;
          if (key === "ArrowDown" || key === "ArrowUp") {
            e.preventDefault();
            e.stopPropagation();
            list[(at + (key === "ArrowDown" ? 1 : -1) + list.length) % list.length]?.focus();
          } else if (key === "ArrowRight") {
            e.preventDefault();
            e.stopPropagation();
            const el = document.activeElement as HTMLElement;
            const i = Number(el?.dataset.index);
            const ent = entries[i];
            if (ent && ent !== "-" && ent.sub) openSub(i, el, true);
          } else if (key === "ArrowLeft" || key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            if (depth > 0) onBack?.();
            else if (key === "Escape") onDone();
          } else if (key.length === 1 && !e.ctrlKey && !e.altKey) {
            const k = key.toLowerCase();
            const i = entries.findIndex((en) => en !== "-" && !en.disabled && accessKey(en.label) === k);
            if (i >= 0) {
              e.preventDefault();
              e.stopPropagation();
              const el = ref.current?.querySelector<HTMLElement>(`:scope > button[data-index="${i}"]`);
              const en = entries[i] as Exclude<MenuEntry, "-">;
              if (en.sub && el) openSub(i, el, true);
              else (onDone(), en.run?.());
            }
          }
        }}
      >
        {entries.map((en, i) =>
          en === "-" ? (
            <div key={i} className="pm__sep" role="separator" />
          ) : (
            <button
              key={i}
              role="menuitem"
              data-index={i}
              disabled={en.disabled}
              aria-haspopup={en.sub ? "menu" : undefined}
              aria-expanded={en.sub ? open?.i === i : undefined}
              aria-checked={en.checked != null ? en.checked : undefined}
              className={`pm__item ${open?.i === i ? "is-open" : ""}`}
              onMouseEnter={(e) => {
                window.clearTimeout(hoverTimer.current);
                const el = e.currentTarget;
                hoverTimer.current = window.setTimeout(() => (en.sub ? openSub(i, el, false) : setOpen(null)), en.sub ? 120 : 200);
              }}
              onClick={(e) => {
                if (en.sub) return openSub(i, e.currentTarget, true);
                onDone();
                en.run?.();
              }}
            >
              <span className="pm__check">{en.checked ? "✓" : ""}</span>
              <span className="pm__label">
                <Label text={en.label} />
              </span>
              <span className="pm__arrow">{en.sub ? "›" : ""}</span>
            </button>
          ),
        )}
      </div>
      {open && (() => {
        const en = entries[open.i];
        return en !== "-" && en.sub ? (
          <Level
            entries={en.sub}
            x={open.x}
            y={open.y}
            flipFrom={open.left}
            depth={depth + 1}
            onDone={onDone}
            onBack={() => {
              setOpen(null);
              ref.current?.querySelector<HTMLElement>(`:scope > button[data-index="${open.i}"]`)?.focus();
            }}
          />
        ) : null;
      })()}
    </>
  );
}

export function PopupMenu({ x, y, entries, onClose }: { x: number; y: number; entries: MenuEntry[]; onClose: () => void }) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: MouseEvent) => !root.current?.contains(e.target as Node) && onClose();
    const gone = () => onClose();
    window.addEventListener("mousedown", away);
    window.addEventListener("blur", gone);
    window.addEventListener("resize", gone);
    return () => (window.removeEventListener("mousedown", away), window.removeEventListener("blur", gone), window.removeEventListener("resize", gone));
  }, [onClose]);
  return (
    <div ref={root} className="pm-root">
      <Level entries={entries} x={x} y={y} depth={0} onDone={onClose} />
    </div>
  );
}
