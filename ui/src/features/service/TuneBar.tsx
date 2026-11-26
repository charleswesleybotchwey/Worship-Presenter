import { useEffect, useRef, useState } from "react";
import { api, type MediaItem, type Slide } from "../../api";
import { useMediaUrl } from "../../theme/media";

// Hymn tunes: a recording of one verse of the tune, played once per verse; the slides of
// each verse move in step with it. For a congregation without an organist.

/** The song's verses as runs of slides: each verse with what follows it (a refrain) up to the
 *  next verse. Slides before the first verse (a title slide) are left out. */
export function stanzas(slides: Pick<Slide, "label" | "occurrence">[]): { start: number; end: number }[] {
  const isVerse = (l: string) => /^V\d*$/.test(l);
  const starts: number[] = [];
  slides.forEach((s, i) => {
    const prev = slides[i - 1];
    const newRun = i === 0 || prev.label !== s.label || prev.occurrence !== s.occurrence;
    if (newRun && isVerse(s.label)) starts.push(i);
  });
  if (!starts.length) {
    // no verses (a chorus only): each section is sung to the tune
    slides.forEach((s, i) => {
      const prev = slides[i - 1];
      if (i === 0 || prev.label !== s.label || prev.occurrence !== s.occurrence) starts.push(i);
    });
  }
  return starts.map((start, k) => ({ start, end: (starts[k + 1] ?? slides.length) - 1 }));
}

export function TuneBar({ songId, slides, slideIdx, onShow }: { songId: number; slides: Slide[]; slideIdx: number; onShow: (i: number) => void }) {
  const [tracks, setTracks] = useState<MediaItem[]>([]);
  const [tune, setTune] = useState<{ media_id: number; gap_ms: number } | null>(null);
  const [playing, setPlaying] = useState<{ stanza: number } | null>(null);
  const [waiting, setWaiting] = useState(false);
  const audio = useRef<HTMLAudioElement>(null);
  const shown = useRef<number | null>(null);
  const url = useMediaUrl();
  useEffect(() => {
    Promise.all([api.listMedia("audio", ""), api.listMedia("video", "")])
      .then(([a, v]) => setTracks([...a, ...v]))
      .catch(() => {});
  }, []);
  useEffect(() => {
    setPlaying(null);
    audio.current?.pause();
    api.tuneGet(songId).then(setTune).catch(() => setTune(null));
  }, [songId]);
  const runs = stanzas(slides);
  // during a verse: its slides in step with the tune
  useEffect(() => {
    if (!playing || waiting) return;
    const run = runs[playing.stanza];
    if (!run) return;
    const t = setInterval(() => {
      const a = audio.current;
      if (!a || !a.duration) return;
      const len = run.end - run.start + 1;
      const i = run.start + Math.min(len - 1, Math.floor((a.currentTime / a.duration) * len));
      if (i !== shown.current) {
        shown.current = i;
        onShow(i);
      }
    }, 100);
    return () => clearInterval(t);
  }, [playing, waiting, runs, onShow]);
  const media = tracks.find((m) => m.id === tune?.media_id);
  const startStanza = async (k: number) => {
    const a = audio.current;
    if (!a || !runs[k]) return setPlaying(null);
    shown.current = null;
    onShow(runs[k].start);
    a.currentTime = 0;
    setPlaying({ stanza: k });
    await a.play().catch(() => setPlaying(null));
  };
  const play = () => {
    // from the verse on the screens, else the first
    const k = Math.max(0, runs.findIndex((r) => slideIdx >= r.start && slideIdx <= r.end));
    void startStanza(k);
  };
  const ended = () => {
    if (!playing) return;
    const next = playing.stanza + 1;
    if (next >= runs.length) return setPlaying(null);
    setWaiting(true);
    setTimeout(() => {
      setWaiting(false);
      void startStanza(next);
    }, tune?.gap_ms ?? 1500);
  };
  const stop = () => {
    audio.current?.pause();
    setPlaying(null);
  };
  return (
    <div className="tune" aria-label="Hymn tune">
      <span className="tlb__label">Tune</span>
      <select
        className="wp-input tlb__track-like tune__track"
        value={tune?.media_id ?? ""}
        disabled={!!playing}
        aria-label="The song's tune"
        onChange={(e) => {
          const id = e.target.value ? Number(e.target.value) : null;
          const t = id == null ? null : { media_id: id, gap_ms: tune?.gap_ms ?? 1500 };
          setTune(t);
          void api.tuneSet(songId, t);
        }}
      >
        <option value="">No tune (one verse of it, from the media library)…</option>
        {tracks.map((m) => (
          <option key={m.id} value={m.id}>
            {m.kind === "video" ? "🎞 " : "♪ "}
            {m.title}
          </option>
        ))}
      </select>
      {!playing ? (
        <button className="wp-btn" disabled={!media || runs.length === 0} onClick={play} title="The tune once per verse; the slides move with it">
          ▶ Play verse by verse
        </button>
      ) : (
        <button className="wp-btn wp-btn--primary" onClick={stop}>
          ■ Stop
        </button>
      )}
      {playing && (
        <span className="tune__time">
          {waiting ? "breath…" : `verse ${playing.stanza + 1} of ${runs.length}`}
        </span>
      )}
      {tune && !playing && (
        <label className="tune__gap" title="The pause between verses">
          Breath
          <input
            className="wp-input"
            type="number"
            min={0}
            max={10}
            step={0.5}
            value={tune.gap_ms / 1000}
            onChange={(e) => {
              const t = { ...tune, gap_ms: Math.round((Number(e.target.value) || 0) * 1000) };
              setTune(t);
              void api.tuneSet(songId, t);
            }}
            aria-label="Breath between verses (seconds)"
          />
          s
        </label>
      )}
      {media && <audio ref={audio} src={url(`media:${media.path}`)} preload="auto" onEnded={ended} />}
    </div>
  );
}
