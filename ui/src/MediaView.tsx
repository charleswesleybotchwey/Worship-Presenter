import { useEffect, useRef } from "react";
import type { MediaItem } from "./api";
import { useMediaUrl } from "./theme/media";

// A picture or video shown full screen as a live item. The video follows the core's
// play/pause and restart state; only the main output plays sound.

export function MediaView({ item, playing = true, restart = 0, muted = true, still = false }: { item: MediaItem; playing?: boolean; restart?: number; muted?: boolean; still?: boolean }) {
  const url = useMediaUrl();
  const v = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const el = v.current;
    if (!el) return;
    if (playing) el.play().catch(() => {});
    else el.pause();
  }, [playing]);

  useEffect(() => {
    const el = v.current;
    if (!el) return;
    el.currentTime = 0;
    if (playing) el.play().catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [restart]);

  const src = url(`media:${item.path}`);
  const thumb = item.thumb ? url(item.thumb.startsWith("data:") ? item.thumb : `media:${item.thumb}`) : undefined;
  return (
    <div className="tv tv--media">
      {item.kind === "image" && <img className="tv__bg" src={src} alt="" style={{ objectFit: "contain" }} />}
      {item.kind === "video" && (still ? thumb && <img className="tv__bg" src={thumb} alt="" style={{ objectFit: "contain" }} /> : <video ref={v} className="tv__bg" src={src} style={{ objectFit: "contain" }} autoPlay muted={muted} playsInline />)}
      {item.kind === "audio" && !still && <audio ref={v as unknown as React.RefObject<HTMLAudioElement>} src={src} autoPlay muted={muted} />}
      {item.kind === "audio" && <div className="media-audio">♪ {item.title}</div>}
    </div>
  );
}
