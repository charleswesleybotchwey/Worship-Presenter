import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Control } from "./Control";
import { Output } from "./Output";
import { Fixture } from "./ui/Fixture";
import { ThemesProvider } from "./theme/store";
import { MediaUrlProvider } from "./theme/media";
import "./ui/tokens.css";
import "./styles.css";
import "./ui/shell.css";

// #/output/<role> renders an output window; anything else is the control window.
const m = location.hash.match(/^#\/output\/([a-z_]+)/);
// Operator appearance applies to the control window only; outputs never read it.
if (!m) document.documentElement.dataset.theme = (() => {
  const t = localStorage.getItem("wp-ui-theme") || "dark";
  return t === "system" ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light") : t;
})();

createRoot(document.getElementById("root")!).render(<StrictMode>
    <MediaUrlProvider>
      <ThemesProvider>{m ? <Output role={m[1]} /> : location.hash === "#/fixture" ? <Fixture /> : <Control />}</ThemesProvider>
    </MediaUrlProvider>
  </StrictMode>);
