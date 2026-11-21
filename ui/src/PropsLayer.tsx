import { useEffect, useState } from "react";
import { api, onPropsChanged, type MediaItem, type Prop } from "./api";
import { useMediaUrl } from "./theme/media";

// Props on an output (as ProPresenter): graphics over the slides, independently of them.
// `stream`: this screen is a lower third (the livestream); otherwise an audience screen.

function useProps(): Prop[] {
  const [all, setAll] = useState<Prop[]>([]);
  useEffect(() => {
    const load = () => api.propsList().then(setAll).catch(() => {});
    load();
    let off = () => {};
    onPropsChanged(load).then((u) => (off = u));
    return () => off();
  }, []);
  return all;
}

function useClockText() {
  const [t, setT] = useState(() => new Date());
  useEffect(() => {
    const i = setInterval(() => setT(new Date()), 1000);
    return () => clearInterval(i);
  }, []);
  return t.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function PropsLayer({ showing, stream }: { showing: number[]; stream: boolean }) {
  const all = useProps();
  const url = useMediaUrl();
  const clock = useClockText();
  const [media, setMedia] = useState<MediaItem[]>([]);
  const needPictures = all.some((p) => p.kind === "image" && showing.includes(p.id ?? -1));
  useEffect(() => {
    if (needPictures) Promise.all([api.listMedia("image", ""), api.listMedia("video", "")]).then(([a, b]) => setMedia([...a, ...b])).catch(() => {});
  }, [needPictures]);
  const shown = all.filter((p) => p.id != null && showing.includes(p.id) && (p.on === "all" || (p.on === "stream") === stream));
  if (!shown.length) return null;
  return (
    <div className="props-layer" aria-hidden="true">
      {shown.map((p) => {
        const style = { opacity: p.opacity, color: p.color, fontSize: `${p.size}cqh` } as React.CSSProperties;
        const pic = p.kind === "image" ? media.find((m) => m.id === p.media_id) : undefined;
        return (
          <div key={p.id} className={`prop prop--${p.position}`} style={style}>
            {p.kind === "image" ? (
              pic &&
              (pic.kind === "video" ? (
                // a looping video prop (an animated logo, a moving badge): two videos at once
                <video src={url(`media:${pic.path}`)} autoPlay muted loop playsInline style={{ width: `${p.size}cqw` }} />
              ) : (
                <img src={url(`media:${pic.path}`)} alt="" style={{ width: `${p.size}cqw` }} />
              ))
            ) : (
              <span className="prop__text" style={{ background: p.background || undefined }}>
                {p.kind === "clock" ? clock : p.text}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}
