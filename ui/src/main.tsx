import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Control } from "./Control";
import { Output } from "./Output";
import { Fixture } from "./ui/Fixture";

const ident = location.hash.match(/^#\/identify\/(\d+)\/([^/]*)\/?(.*)$/);

function Identify({ n, res, name }: { n: string; res: string; name: string }) {
  return (
    <div className="ident">
      <div className="ident__n">{n}</div>
      <div className="ident__name">
        Monitor {n}
        {name ? ` · ${name}` : ""}
      </div>
      <div className="ident__res">{res.replace("x", " × ")}</div>
    </div>
  );
}
import { ThemesProvider } from "./theme/store";
import { MediaUrlProvider } from "./theme/media";
import "./ui/tokens.css";
import "./styles.css";
import "./ui/shell.css";
import "./ui/depth.css";
import { installNativeGuard } from "./nativeGuard";

installNativeGuard();

// errors in any window go to the app's log (Settings → About → Open the logs folder)
{
  const where = location.hash.replace(/^#\//, "") || "control";
  let sent = 0;
  const report = (text: string) => {
    if (sent++ < 50) void import("./api").then(({ api }) => api.logError(where, text).catch(() => {}));
  };
  window.addEventListener("error", (e) => report(`${e.message} (${e.filename?.split("/").pop()}:${e.lineno})`));
  window.addEventListener("unhandledrejection", (e) => report(`unhandled: ${e.reason instanceof Error ? e.reason.message : String(e.reason)}`));
}

// #/output/<role> renders an output window; anything else is the control window.
const m = location.hash.match(/^#\/output\/([a-z_]+)/);
// Operator appearance applies to the control window only; outputs never read it.
document.documentElement.dataset.depth = localStorage.getItem("wp-depth") || "3d";
if (!m && !ident) document.documentElement.dataset.theme = (() => {
  const t = localStorage.getItem("wp-ui-theme") || "dark";
  return t === "system" ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light") : t;
})();

if (ident) {
  createRoot(document.getElementById("root")!).render(<Identify n={ident[1]} res={ident[2]} name={decodeURIComponent(ident[3] ?? "")} />);
} else
createRoot(document.getElementById("root")!).render(<StrictMode>
    <MediaUrlProvider>
      <ThemesProvider>
        {m ? <Output role={m[1]} /> : location.hash === "#/fixture" ? <Fixture /> : <Control />}
      </ThemesProvider>
    </MediaUrlProvider>
  </StrictMode>);
