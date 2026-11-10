import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { mediaUrlResolver, onProfileChanged, resetMediaRoot, api, type MediaItem } from "../api";

// Themes refer to library files as "media:videos/loop.mp4"; this turns that into a
// URL the window can load. Plain data: URLs (pictures stored inside a theme) pass through.

const Ctx = createContext<(src: string) => string>((s) => s);

export function MediaUrlProvider({ children }: { children: ReactNode }) {
  const [resolve, setResolve] = useState<(rel: string) => string>(() => (s: string) => s);
  useEffect(() => {
    const load = () => mediaUrlResolver().then((f) => setResolve(() => f));
    load();
    let stop = () => {};
    onProfileChanged(() => (resetMediaRoot(), load())).then((u) => (stop = u));
    return () => stop();
  }, []);
  const fn = (src: string) => (src.startsWith("media:") ? resolve(src.slice(6)) : src);
  return <Ctx.Provider value={fn}>{children}</Ctx.Provider>;
}

export const useMediaUrl = () => useContext(Ctx);

/** Makes a 320×180 JPEG thumbnail (and reads a video's length and size) after import. */
export async function makeThumb(item: MediaItem, url: string): Promise<void> {
  if (item.kind === "audio") return;
  const c = document.createElement("canvas");
  c.width = 320;
  c.height = 180;
  const g = c.getContext("2d")!;
  const draw = (el: CanvasImageSource, w: number, h: number) => {
    const s = Math.max(320 / w, 180 / h);
    g.fillStyle = "#000";
    g.fillRect(0, 0, 320, 180);
    g.drawImage(el, (320 - w * s) / 2, (180 - h * s) / 2, w * s, h * s);
  };
  if (item.kind === "image") {
    const img = new Image();
    img.src = url;
    await img.decode();
    draw(img, img.naturalWidth, img.naturalHeight);
    await api.setMediaMeta(item.id, null, img.naturalWidth, img.naturalHeight);
  } else {
    const v = document.createElement("video");
    v.muted = true;
    v.preload = "auto";
    v.src = url;
    await new Promise<void>((ok, fail) => ((v.onloadedmetadata = () => ok()), (v.onerror = () => fail(new Error("This video cannot be played on this computer.")))));
    const d = Number.isFinite(v.duration) ? v.duration : 0;
    v.currentTime = Math.min(1, d / 3);
    await new Promise<void>((ok) => (v.onseeked = () => ok()));
    draw(v, v.videoWidth, v.videoHeight);
    await api.setMediaMeta(item.id, Math.round(d * 1000), v.videoWidth, v.videoHeight);
    v.removeAttribute("src");
    v.load();
  }
  const blob: Blob = await new Promise((ok) => c.toBlob((b) => ok(b!), "image/jpeg", 0.8));
  await api.saveMediaThumb(item.id, Array.from(new Uint8Array(await blob.arrayBuffer())));
}
