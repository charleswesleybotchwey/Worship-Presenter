// Makes the app behave like a Windows program rather than a web page: no browser
// right-click menu, no reload/print/find/back/zoom keys, no pinch zoom, and pictures
// and links cannot be dragged out. Installed in every window (control and outputs).
// It only cancels the browser's default; the app's own key handling still runs.

const BROWSER_KEYS = new Set(["r", "p", "f", "g", "u", "j", "h", "o", "n", "t", "w", "s", "l", "d", "e", "+", "=", "-", "_", "0"]);

export function installNativeGuard() {
  // the app's own context menus open from React handlers; the browser's never does.
  // Text fields keep Windows' Cut / Copy / Paste menu.
  window.addEventListener("contextmenu", (e) => {
    const t = e.target as HTMLElement | null;
    const editable = !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
    if (!editable) e.preventDefault();
  });
  window.addEventListener(
    "keydown",
    (e) => {
      const k = e.key.toLowerCase();
      const ctrl = e.ctrlKey || e.metaKey;
      const browser =
        (ctrl && BROWSER_KEYS.has(k)) || // reload, print, find, view source, downloads, history, zoom…
        (ctrl && e.shiftKey && ["i", "j", "c", "delete"].includes(k)) || // developer tools, clear data
        k === "f12" || k === "f7" || k === "f3" || // dev tools, caret browsing, find next
        (e.altKey && (k === "arrowleft" || k === "arrowright" || k === "home")) || // back / forward / home
        k === "browserback" || k === "browserforward" || k === "browserrefresh" || k === "browsersearch";
      // keep normal text editing shortcuts inside fields (Ctrl+A/C/V/X/Z work everywhere)
      if (browser) e.preventDefault();
    },
    { capture: true },
  );
  // Ctrl + mouse wheel and touchpad pinch (which arrives as Ctrl + wheel) zoom pages
  window.addEventListener("wheel", (e) => e.ctrlKey && e.preventDefault(), { passive: false });
  // pictures and links are not dragged out as ghost images; our own drag-and-drop
  // rows are marked draggable and still work
  window.addEventListener("dragstart", (e) => {
    const t = e.target as HTMLElement | null;
    if (t && !t.closest?.("[draggable=true]")) e.preventDefault();
  });
}
