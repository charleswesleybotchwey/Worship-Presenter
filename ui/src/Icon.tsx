// One line-icon set: 16px grid, 1.5px stroke, round joins. Drawn for this app.
const PATHS: Record<string, string> = {
  globe: "M8 1.5a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13M1.5 8h13M8 1.5c1.9 1.9 2.7 4.2 2.7 6.5S9.9 12.6 8 14.5M8 1.5C6.1 3.4 5.3 5.7 5.3 8S6.1 12.6 8 14.5",
  heading: "M2.5 4h11M2.5 8h7M2.5 12h11",
  layers: "M8 2l6 3.2-6 3.2-6-3.2zM2 8.4l6 3.2 6-3.2M2 11.4l6 3.2 6-3.2",
  badge: "M2.5 3.5h11v7h-11zM5.5 13.5h5M8 10.5v3",
  bolt: "M9 1.5L3.5 9H8l-1 5.5L12.5 7H8z",
  take: "M2.5 8h10M8.5 4l4 4-4 4",
  clearall: "M3 3l10 10M13 3L3 13M8 1.5v2M8 12.5v2",

  search: "M7 12.5a5.5 5.5 0 1 0 0-11 5.5 5.5 0 0 0 0 11ZM11 11l3.5 3.5",
  plus: "M8 3v10M3 8h10",
  close: "M4 4l8 8M12 4l-8 8",
  note: "M6 12.5V3.5l7-1.5v9M6 12.5a2 2 0 1 1-4 0 2 2 0 0 1 4 0ZM13 11a2 2 0 1 1-4 0 2 2 0 0 1 4 0Z",
  book: "M2.5 3.5c2-.8 3.8-.8 5.5.5v9c-1.7-1.3-3.5-1.3-5.5-.5v-9ZM13.5 3.5c-2-.8-3.8-.8-5.5.5v9c1.7-1.3 3.5-1.3 5.5-.5v-9Z",
  monitor: "M2 3h12v8H2zM6 14h4M8 11v3",
  import: "M8 2v8M5 7l3 3 3-3M3 13h10",
  video: "M2 4h9v8H2zM11 7l3-2v6l-3-2",
  mic: "M8 1.5a2 2 0 0 0-2 2V8a2 2 0 0 0 4 0V3.5a2 2 0 0 0-2-2ZM4 7.5a4 4 0 0 0 8 0M8 11.5V14.5M5.5 14.5h5",
  sun: "M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM8 1.5v1.5M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1 1M11.6 11.6l1 1M3.4 12.6l1-1M11.6 4.4l1-1",
  moon: "M13.5 9.5A5.5 5.5 0 0 1 6.5 2.5a5.5 5.5 0 1 0 7 7Z",
  settings: "M8 10.25a2.25 2.25 0 1 0 0-4.5 2.25 2.25 0 0 0 0 4.5ZM8 1.5l1.1 1.6 1.9-.5.4 1.9 1.8.8-.6 1.8 1.1 1.6-1.6 1.1.2 1.9-1.9.3-.9 1.7-1.7-.9L8 14.5l-1.1-1.6-1.9.5-.4-1.9-1.8-.8.6-1.8L2.3 7.3l1.6-1.1-.2-1.9 1.9-.3.9-1.7 1.7.9Z",
  music: "M6 12V3.5l7-1.5v8.5M6 12a1.75 1.75 0 1 1-3.5 0A1.75 1.75 0 0 1 6 12ZM13 10.5a1.75 1.75 0 1 1-3.5 0 1.75 1.75 0 0 1 3.5 0Z",
  slides: "M2 3.5h12v8H2zM5 14h6M4.5 6.5h7M4.5 8.5h4",
  more: "M3.5 8h.01M8 8h.01M12.5 8h.01",
  speaker: "M2.5 6h2.5L8.5 3v10L5 10H2.5V6ZM11 5.5a3.5 3.5 0 0 1 0 5",
  check: "M3 8.5l3 3 7-7",
  blank: "M2.5 3.5h11v9h-11z",
  logo: "M2.5 3.5h11v9h-11zM6 8a2 2 0 1 0 4 0 2 2 0 0 0-4 0",
  clear: "M2.5 3.5h11v9h-11zM5 10.5l6-5",
  timer: "M8 14A5.5 5.5 0 1 0 8 3a5.5 5.5 0 0 0 0 11ZM8 5.5V8.5l2 1.5M6.5 1.5h3",
  alert: "M8 2l6.5 11.5h-13L8 2ZM8 6.5v3.2M8 11.6h.01",
  image: "M2 3h12v10H2zM2 11l3.5-3.5 3 3 2-2L14 12M10.5 6.5h.01",
  theme: "M2 3h12v10H2zM4.5 9.5h7M5.5 11.5h5M2 6h12",
  edit: "M10.5 2.5l3 3L6 13H3v-3l7.5-7.5Z",
  backup: "M3 4.5C3 3.7 5.2 3 8 3s5 .7 5 1.5v7c0 .8-2.2 1.5-5 1.5s-5-.7-5-1.5v-7ZM3 4.5c0 .8 2.2 1.5 5 1.5s5-.7 5-1.5M3 8c0 .8 2.2 1.5 5 1.5S13 8.8 13 8",
};

export function Icon({ name }: { name: keyof typeof PATHS | string }) {
  return (
    <svg className="icon" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={PATHS[name] ?? ""} />
    </svg>
  );
}
