// One line-icon set: 16px grid, 1.5px stroke, round joins. Drawn for this app.
const PATHS: Record<string, string> = {
  search: "M7 12.5a5.5 5.5 0 1 0 0-11 5.5 5.5 0 0 0 0 11ZM11 11l3.5 3.5",
  plus: "M8 3v10M3 8h10",
  close: "M4 4l8 8M12 4l-8 8",
  note: "M6 12.5V3.5l7-1.5v9M6 12.5a2 2 0 1 1-4 0 2 2 0 0 1 4 0ZM13 11a2 2 0 1 1-4 0 2 2 0 0 1 4 0Z",
  book: "M2.5 3.5c2-.8 3.8-.8 5.5.5v9c-1.7-1.3-3.5-1.3-5.5-.5v-9ZM13.5 3.5c-2-.8-3.8-.8-5.5.5v9c1.7-1.3 3.5-1.3 5.5-.5v-9Z",
  monitor: "M2 3h12v8H2zM6 14h4M8 11v3",
};

export function Icon({ name }: { name: keyof typeof PATHS | string }) {
  return (
    <svg className="icon" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={PATHS[name] ?? ""} />
    </svg>
  );
}
