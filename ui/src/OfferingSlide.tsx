import { useEffect, useRef, useState } from "react";
import qrcode from "qrcode-generator";
import { api } from "./api";
import { useBrand } from "./theme/brand";

// An offering slide in the church's colours: mobile money and bank details, and a QR code
// holding exactly what the church pastes (its official GhQR text or a payment link).
// No payment codes are made up here: a wrong code would send giving to the wrong place.

interface Line {
  network: string;
  number: string;
  name: string;
}

const NETWORKS = ["MTN MoMo", "Telecel Cash", "AirtelTigo Money", "Bank"];

function load(): { title: string; lines: Line[]; qr: string; message: string } {
  try {
    return { title: "Tithes and Offering", lines: [{ network: "MTN MoMo", number: "", name: "" }], qr: "", message: "God loves a cheerful giver. 2 Corinthians 9:7", ...JSON.parse(localStorage.getItem("wp-offering") || "{}") };
  } catch {
    return { title: "Tithes and Offering", lines: [{ network: "MTN MoMo", number: "", name: "" }], qr: "", message: "" };
  }
}

function draw(c: HTMLCanvasElement, d: ReturnType<typeof load>, colors: string[], font: string, heading: string) {
  const W = 1920;
  const H = 1080;
  c.width = W;
  c.height = H;
  const g = c.getContext("2d")!;
  const [main, accent, text, bg] = colors;
  const grad = g.createLinearGradient(0, 0, W, H);
  grad.addColorStop(0, bg);
  grad.addColorStop(1, main);
  g.fillStyle = grad;
  g.fillRect(0, 0, W, H);
  g.fillStyle = accent;
  g.fillRect(120, 230, 160, 10);
  g.fillStyle = text;
  g.textBaseline = "alphabetic";
  g.font = `800 96px "${heading}", "Noto Sans", sans-serif`;
  g.fillText(d.title, 120, 200);
  const hasQr = d.qr.trim().length > 0;
  const textW = hasQr ? 1080 : 1680;
  let y = 360;
  for (const l of d.lines.filter((x) => x.number.trim())) {
    g.fillStyle = accent;
    g.font = `700 44px "${heading}", "Noto Sans", sans-serif`;
    g.fillText(l.network, 120, y);
    g.fillStyle = text;
    g.font = `800 72px "${font}", "Noto Sans", sans-serif`;
    g.fillText(l.number, 120, y + 80, textW);
    if (l.name.trim()) {
      g.font = `500 40px "${font}", "Noto Sans", sans-serif`;
      g.globalAlpha = 0.85;
      g.fillText(l.name, 120, y + 132, textW);
      g.globalAlpha = 1;
    }
    y += 210;
  }
  if (d.message.trim()) {
    g.fillStyle = text;
    g.globalAlpha = 0.85;
    g.font = `italic 500 40px "${font}", "Noto Sans", sans-serif`;
    g.fillText(d.message, 120, H - 90, 1680);
    g.globalAlpha = 1;
  }
  if (hasQr) {
    const q = qrcode(0, "M");
    q.addData(d.qr.trim());
    q.make();
    const n = q.getModuleCount();
    const size = 560;
    const cell = Math.floor(size / (n + 8));
    const box = cell * (n + 8);
    const x0 = W - 120 - box;
    const y0 = 300;
    g.fillStyle = "#ffffff";
    g.fillRect(x0, y0, box, box);
    g.fillStyle = "#000000";
    for (let r = 0; r < n; r++) for (let k = 0; k < n; k++) if (q.isDark(r, k)) g.fillRect(x0 + (k + 4) * cell, y0 + (r + 4) * cell, cell, cell);
    g.fillStyle = text;
    g.font = `600 36px "${font}", "Noto Sans", sans-serif`;
    g.textAlign = "center";
    g.fillText("Scan to give", x0 + box / 2, y0 + box + 56);
    g.textAlign = "left";
  }
}

export function OfferingSlide({ onClose, onMade }: { onClose: () => void; onMade: (title: string) => void }) {
  const [d, setD] = useState(load);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const brand = useBrand();
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (canvas.current) {
      try {
        draw(canvas.current, d, brand.colors, brand.font, brand.heading);
        setProblem(null);
      } catch {
        setProblem("That QR text is too long for a QR code.");
      }
    }
  }, [d, brand]);
  const setLine = (i: number, p: Partial<Line>) => setD((x) => ({ ...x, lines: x.lines.map((l, j) => (j === i ? { ...l, ...p } : l)) }));
  const make = async () => {
    if (!d.lines.some((l) => l.number.trim()) && !d.qr.trim()) return setProblem("Add at least one number or a QR code.");
    setBusy(true);
    try {
      localStorage.setItem("wp-offering", JSON.stringify(d));
      const c = document.createElement("canvas");
      draw(c, d, brand.colors, brand.font, brand.heading);
      const blob: Blob = await new Promise((ok) => c.toBlob((b) => ok(b!), "image/png"));
      const token = Date.now().toString(16);
      await api.presentationPage(token, 1, new Uint8Array(await blob.arrayBuffer()), "png");
      await api.presentationFinish(token, d.title || "Offering");
      onMade(d.title || "Offering");
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog wp-dialog--wide off" role="dialog" aria-modal="true" aria-label="Offering slide">
        <header className="wp-dialog__head">
          <h2>Offering slide</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        <div className="off__body">
          <div className="off__form">
            <label className="ps__row">
              <span>Title</span>
              <input className="wp-input" value={d.title} onChange={(e) => setD({ ...d, title: e.target.value })} aria-label="Title" />
            </label>
            {d.lines.map((l, i) => (
              <div key={i} className="off__line">
                <select className="wp-input" value={l.network} onChange={(e) => setLine(i, { network: e.target.value })} aria-label="Network">
                  {NETWORKS.map((n) => (
                    <option key={n}>{n}</option>
                  ))}
                </select>
                <input className="wp-input" placeholder="Number or merchant ID" value={l.number} onChange={(e) => setLine(i, { number: e.target.value })} aria-label="Number" />
                <input className="wp-input" placeholder="Account name" value={l.name} onChange={(e) => setLine(i, { name: e.target.value })} aria-label="Account name" />
                <button className="wp-icon-btn" aria-label="Remove" onClick={() => setD({ ...d, lines: d.lines.filter((_, j) => j !== i) })}>
                  ✕
                </button>
              </div>
            ))}
            {d.lines.length < 3 && (
              <button className="btn-quiet" onClick={() => setD({ ...d, lines: [...d.lines, { network: NETWORKS[d.lines.length] ?? "Bank", number: "", name: "" }] })}>
                Add another way to give
              </button>
            )}
            <label className="ps__row">
              <span>QR code (optional)</span>
              <input className="wp-input" placeholder="Paste the official QR text or a payment link" value={d.qr} onChange={(e) => setD({ ...d, qr: e.target.value })} aria-label="QR code text" />
            </label>
            <p className="wp-note">Use the QR text your bank or MoMo merchant account gives you (GhQR), or a payment link (Hubtel, Paystack…). Scan the preview with a phone before Sunday.</p>
            <label className="ps__row">
              <span>Line at the bottom</span>
              <input className="wp-input" value={d.message} onChange={(e) => setD({ ...d, message: e.target.value })} aria-label="Line at the bottom" />
            </label>
            {problem && <p className="form-error">{problem}</p>}
            <button className="wp-btn wp-btn--primary" disabled={busy} onClick={make}>
              {busy ? "Making the slide…" : "Create slide"}
            </button>
          </div>
          <canvas ref={canvas} className="off__preview" aria-label="Preview" />
        </div>
      </div>
    </div>
  );
}
