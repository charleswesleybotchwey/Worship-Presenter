import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Control } from "./Control";
import { Output } from "./Output";
import "./styles.css";

// #/output/<role> renders an output window; anything else is the control window.
const m = location.hash.match(/^#\/output\/([a-z_]+)/);

createRoot(document.getElementById("root")!).render(<StrictMode>{m ? <Output role={m[1]} /> : <Control />}</StrictMode>);
