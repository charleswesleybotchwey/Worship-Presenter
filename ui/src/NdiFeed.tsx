import { useEffect, useRef, useState } from "react";
import { ndiFrameUrl } from "./api";

/** A live NDI source: pulls the newest picture as fast as it arrives and draws it on a
 *  canvas (no flicker between pictures). `id` is "ndi:<name>\u0001<address>". */
export function NdiFeed({ id, className }: { id: string; className?: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    const [name, url] = id.slice(4).split("\u0001");
    let alive = true;
    let n = 0;
    let misses = 0;
    let base = "";
    const next = () => {
      if (!alive) return;
      const img = new Image();
      img.onload = () => {
        misses = 0;
        setProblem(null);
        const c = canvas.current;
        if (c) {
          if (c.width !== img.naturalWidth) c.width = img.naturalWidth;
          if (c.height !== img.naturalHeight) c.height = img.naturalHeight;
          c.getContext("2d")?.drawImage(img, 0, 0);
        }
        setTimeout(next, 15);
      };
      img.onerror = () => {
        if (++misses > 4) setProblem(`${name}: no picture`);
        setTimeout(next, 500);
      };
      img.src = `${base}${base.includes("?") ? "&" : "?"}n=${n++}`;
    };
    ndiFrameUrl(name, url ?? "").then((u) => ((base = u), next()));
    return () => {
      alive = false;
    };
  }, [id]);
  return (
    <div className={className}>
      <canvas ref={canvas} style={{ width: "100%", height: "100%", objectFit: "cover", display: "block", background: "#000" }} />
      {problem && <span style={{ position: "absolute", left: "50%", top: "50%", transform: "translate(-50%,-50%)", color: "#888", font: "500 13px sans-serif" }}>{problem}</span>}
    </div>
  );
}
