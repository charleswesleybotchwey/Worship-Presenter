import { useEffect, useRef, useState } from "react";
import { api, type MediaItem, type ProgramItem, type Slide } from "../../api";
import { useMediaUrl } from "../../theme/media";

// Timelines (as ProPresenter): a song or deck in time with a backing track. Record: the
// track plays and every slide change is noted against its time. Play: the slides change by
// themselves at those moments (the operator can still take over).

type Tl = { item_kind: string; item_id: number; media_id: number; cues: [number, number][] };

export function slideAt(cues: [number, number][], ms: number): number | null {
  let s: number | null = null;
  for (const [t, i] of cues) {
    if (t > ms) break;
    s = i;
  }
  return s;
}

const fmt = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

export function TimelineBar({ item, slideIdx, slides, onShow }: { item: ProgramItem | null | undefined; slideIdx: number; slides: Slide[]; onShow: (i: number) => void }) {
  const key = item?.kind === "song" ? (["song", item.song_id] as const) : item?.kind === "announcement" ? (["announcement", item.presentation_id] as const) : null;
  const [tl, setTl] = useState<Tl | null>(null);
  const [tracks, setTracks] = useState<MediaItem[]>([]);
  const [track, setTrack] = useState<number | null>(null);
  const [mode, setMode] = useState<"idle" | "recording" | "playing">("idle");
  const [pos, setPos] = useState(0);
  const [dur, setDur] = useState(0);
  const audio = useRef<HTMLAudioElement>(null);
  const cues = useRef<[number, number][]>([]);
  const lastShown = useRef<number | null>(null);
  const url = useMediaUrl();
  const keyStr = key ? key.join(":") : "";
  useEffect(() => {
    setMode("idle");
    audio.current?.pause();
    if (!key) return void setTl(null);
    api.timelineGet(key[0], key[1]).then((t) => (setTl(t), setTrack(t?.media_id ?? null))).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [keyStr]);
  useEffect(() => {
    Promise.all([api.listMedia("audio", ""), api.listMedia("video", "")])
      .then(([a, v]) => setTracks([...a, ...v]))
      .catch(() => {});
  }, []);
  // recording: each slide change, at the track's time
  useEffect(() => {
    // only a real change (not the same slide again)
    const last = cues.current[cues.current.length - 1];
    if (mode === "recording" && audio.current && last?.[1] !== slideIdx) cues.current.push([Math.round(audio.current.currentTime * 1000), slideIdx]);
  }, [slideIdx, mode]);
  // playing: the slide for the track's time
  useEffect(() => {
    if (mode !== "playing" || !tl) return;
    const t = setInterval(() => {
      const ms = (audio.current?.currentTime ?? 0) * 1000;
      const i = slideAt(tl.cues, ms);
      if (i != null && i !== lastShown.current && i < slides.length) {
        lastShown.current = i;
        onShow(i);
      }
    }, 80);
    return () => clearInterval(t);
  }, [mode, tl, slides.length, onShow]);
  if (!key) return null;
  const media = tracks.find((m) => m.id === track);
  const start = async (m: "recording" | "playing") => {
    const a = audio.current;
    if (!a || !media) return;
    a.currentTime = 0;
    cues.current = m === "recording" ? [[0, slideIdx]] : [];
    lastShown.current = null;
    setMode(m);
    await a.play().catch(() => setMode("idle"));
  };
  const stop = async () => {
    audio.current?.pause();
    if (mode === "recording" && track != null) {
      const t: Tl = { item_kind: key[0], item_id: key[1], media_id: track, cues: cues.current };
      await api.timelineSave(t);
      setTl(t);
    }
    setMode("idle");
  };
  return (
    <div className="tlb" aria-label="Timeline">
      <span className="tlb__label">Timeline</span>
      <select className="wp-input tlb__track" value={track ?? ""} disabled={mode !== "idle"} onChange={(e) => setTrack(e.target.value ? Number(e.target.value) : null)} aria-label="Track">
        <option value="">Choose the track…</option>
        {tracks.map((m) => (
          <option key={m.id} value={m.id}>
            {m.kind === "video" ? "🎞 " : "♪ "}
            {m.title}
          </option>
        ))}
      </select>
      {mode === "idle" ? (
        <>
          <button className="wp-btn tlb__rec" disabled={!media} onClick={() => void start("recording")} title="The track plays: go through the slides as it should go">
            ● Record
          </button>
          <button className="wp-btn" disabled={!media || !tl || tl.media_id !== track} onClick={() => void start("playing")} title="The track plays and the slides change by themselves">
            ▶ Play
          </button>
        </>
      ) : (
        <button className="wp-btn wp-btn--primary" onClick={() => void stop()}>
          ■ Stop{mode === "recording" ? " and save" : ""}
        </button>
      )}
      <div className="tlb__bar" title={tl ? `${tl.cues.length} slide changes recorded` : "Not recorded yet"}>
        <div className="tlb__pos" style={{ width: `${dur ? (pos / dur) * 100 : 0}%` }} />
        {dur > 0 &&
          (mode === "recording" ? cues.current : tl?.cues ?? []).map(([t, i], k) => (
            <span key={k} className="tlb__cue" style={{ left: `${Math.min(100, (t / 1000 / dur) * 100)}%` }} title={`${fmt(t / 1000)} → slide ${i + 1}`} />
          ))}
      </div>
      <span className="tlb__time wp-num">
        {fmt(pos)} / {fmt(dur)}
      </span>
      {tl && mode === "idle" && (
        <button className="btn-quiet" title="Delete this timeline" onClick={() => confirm("Delete this song's timeline?") && void api.timelineDelete(key[0], key[1]).then(() => setTl(null))}>
          ×
        </button>
      )}
      {media && <audio ref={audio} src={url(`media:${media.path}`)} preload="auto" onTimeUpdate={(e) => setPos(e.currentTarget.currentTime)} onLoadedMetadata={(e) => setDur(e.currentTarget.duration || 0)} onEnded={() => void stop()} />}
    </div>
  );
}
