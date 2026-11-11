import { api, mediaUrlResolver } from "../../api";

// PowerPoint and PDF into Presentations. The desktop app converts PowerPoint (with
// PowerPoint or LibreOffice when present); any PDF is drawn here page by page with
// pdf.js (4.x: runs on older WebView2 too) at 1920 pixels wide and handed back as pictures.

export interface ImportResult {
  deckId: number;
  title: string;
  method: string;
  warnings: string[];
}

const METHOD: Record<string, string> = {
  powerpoint: "Converted with PowerPoint: every slide exactly as designed.",
  libreoffice: "Converted with LibreOffice: every slide as a picture. Check fonts and layouts look right.",
  pdf: "Every page of the PDF as a picture.",
  text_only: "Only the words of each slide were kept.",
};
export const methodText = (m: string) => METHOD[m] ?? m;

async function drawPdf(url: string, token: string, onProgress: (text: string) => void) {
  const pdfjs = await import("pdfjs-dist");
  const worker = (await import("pdfjs-dist/build/pdf.worker.min.mjs?url")).default;
  pdfjs.GlobalWorkerOptions.workerSrc = worker;
  // no eval: the app's security policy forbids it
  const task = pdfjs.getDocument({ url, isEvalSupported: false });
  const doc = await task.promise;
  for (let n = 1; n <= doc.numPages; n++) {
    onProgress(`Drawing page ${n} of ${doc.numPages}`);
    const page = await doc.getPage(n);
    const base = page.getViewport({ scale: 1 });
    const viewport = page.getViewport({ scale: 1920 / base.width });
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(viewport.width);
    canvas.height = Math.round(viewport.height);
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = "#fff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    await page.render({ canvasContext: ctx, viewport }).promise;
    const blob: Blob = await new Promise((ok) => canvas.toBlob((b) => ok(b!), "image/jpeg", 0.9));
    await api.presentationPage(token, n, new Uint8Array(await blob.arrayBuffer()), "jpg");
    page.cleanup();
  }
  const pages = doc.numPages;
  await task.destroy();
  return pages;
}

export async function importPresentation(path: string, onProgress: (text: string) => void): Promise<ImportResult> {
  onProgress("Opening the file…");
  const plan = await api.presentationImport(path);
  if (plan.stage === "done") return { deckId: plan.deck_id!, title: plan.title, method: plan.method, warnings: plan.warnings };
  const resolve = await mediaUrlResolver();
  const pages = await drawPdf(resolve(plan.pdf!), plan.token!, onProgress);
  onProgress("Saving the slides…");
  const deckId = await api.presentationFinish(plan.token!, plan.title);
  return { deckId, title: plan.title, method: plan.method, warnings: [...plan.warnings, `${pages} slide${pages === 1 ? "" : "s"}.`] };
}
