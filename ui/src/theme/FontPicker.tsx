import { useEffect, useMemo, useRef, useState } from "react";
import { api, type FontFamily } from "../api";
import { FONTS } from "./types";

// The fonts installed on this computer, each shown in its own face, with a mark for
// whether it has the Ghanaian letters ɛ ɔ ŋ (Noto Sans fills them in otherwise).

let cache: Promise<FontFamily[]> | null = null;
const fallback = (): FontFamily[] => FONTS.map((name) => ({ name, ghanaian: name === "Noto Sans", styles: 1 }));
const installed = () => {
  cache ??= api
    .listFonts()
    .then((l) => (l.length ? l : fallback()))
    .catch(() => fallback());
  return cache;
};

export function FontPicker({ value, onChange }: { value: string; onChange: (f: string) => void }) {
  const [fonts, setFonts] = useState<FontFamily[] | null>(null);
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    void installed().then(setFonts);
  }, []);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);
  const list = useMemo(() => (fonts ?? []).filter((f) => f.name.toLowerCase().includes(q.trim().toLowerCase())), [fonts, q]);
  const current = fonts?.find((f) => f.name.toLowerCase() === value.toLowerCase());
  return (
    <div className="prop font-pick" ref={box}>
      <span>Font</span>
      <button type="button" className="font-pick__btn" onClick={() => setOpen(!open)} aria-haspopup="listbox" aria-expanded={open} style={{ fontFamily: `"${value}", "Noto Sans", sans-serif` }}>
        {value || "Choose a font"}
        <span className="font-pick__caret">▾</span>
      </button>
      {fonts && value && !current && <small className="font-pick__note">Not installed on this computer: Noto Sans is used instead.</small>}
      {current && !current.ghanaian && <small className="font-pick__note">This font has no ɛ ɔ ŋ; those letters come from Noto Sans.</small>}
      {open && (
        <div className="font-pick__menu">
          <input autoFocus className="font-pick__search" placeholder={`Search ${fonts?.length ?? ""} fonts`} value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={(e) => {
            if (e.key === "Escape") setOpen(false);
            if (e.key === "Enter" && list[0]) (onChange(list[0].name), setOpen(false));
          }} />
          <ul role="listbox" aria-label="Installed fonts">
            {list.map((f) => (
              <li key={f.name}>
                <button type="button" role="option" aria-selected={f.name === value} className={f.name === value ? "is-selected" : ""} onClick={() => (onChange(f.name), setOpen(false))}>
                  <span style={{ fontFamily: `"${f.name}", sans-serif` }}>{f.name}</span>
                  <small title={f.ghanaian ? "Has ɛ ɔ ŋ" : "No ɛ ɔ ŋ: Noto Sans fills them in"}>{f.ghanaian ? "ɛ ɔ ✓" : ""}</small>
                </button>
              </li>
            ))}
            {fonts && !list.length && <li className="font-pick__empty">No installed font matches “{q}”.</li>}
            {!fonts && <li className="font-pick__empty">Reading the fonts on this computer…</li>}
          </ul>
        </div>
      )}
    </div>
  );
}
