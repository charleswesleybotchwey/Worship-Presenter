import { useEffect, useState } from "react";
import { api, type MediaItem, type MusicRules, type MusicStatus } from "./api";
import { Icon } from "./Icon";

// Background music: playlist from the media library's sound files, transport,
// volume, and what happens when something goes live.

const mmss = (ms: number | null) => (ms == null ? "" : `${Math.floor(ms / 60000)}:${String(Math.floor((ms % 60000) / 1000)).padStart(2, "0")}`);

export function useMusic() {
  const [m, setM] = useState<MusicStatus | null>(null);
  useEffect(() => {
    const load = () => api.musicStatus().then(setM).catch(() => {});
    load();
    const t = setInterval(load, 1000);
    return () => clearInterval(t);
  }, []);
  return m;
}

export function MusicPanel({ m, onClose }: { m: MusicStatus | null; onClose: () => void }) {
  const [audio, setAudio] = useState<MediaItem[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    api.listMedia("audio", "").then(setAudio).catch(() => {});
  }, []);
  const run = async (f: () => Promise<unknown>) => {
    try {
      await f();
      setProblem(null);
    } catch (e) {
      setProblem(String(e instanceof Error ? e.message : e));
    }
  };
  // local copy so quick successive changes build on each other, not on a stale poll
  const [local, setLocal] = useState<number[] | null>(null);
  const ids = local ?? m?.playlist_ids ?? [];
  const setIds = (n: number[]) => (setLocal(n), run(() => api.musicSetPlaylist(n)));
  const s = m?.status;
  const rules = m?.rules;
  const setRules = (p: Partial<MusicRules>) => rules && run(() => api.musicSetRules({ ...rules, ...p }));
  const move = (i: number, d: number) => {
    const n = [...ids];
    const j = i + d;
    if (j < 0 || j >= n.length) return;
    [n[i], n[j]] = [n[j], n[i]];
    void setIds(n);
  };
  const byId = (id: number) => audio.find((a) => a.id === id);
  const outside = audio.filter((a) => !ids.includes(a.id) && !a.missing);

  return (
    <div className="popover popover--cue" role="dialog" aria-label="Music">
      <div className="cue-now">
        <div className="cue-now__label">{s?.playing ? (s.paused ? "Paused" : s.ducked ? "Playing quietly under the words" : "Playing") : "Not playing"}</div>
        <div className="cue-now__text">{s?.title ?? "Choose music below, then Play."}</div>
        {s?.playing && s.length_ms ? (
          <div className="music-bar" role="progressbar" aria-valuemin={0} aria-valuemax={s.length_ms} aria-valuenow={s.position_ms}>
            <span style={{ width: `${Math.min(100, (s.position_ms / s.length_ms) * 100)}%` }} />
            <em className="wp-num">
              {mmss(s.position_ms)} / {mmss(s.length_ms)}
            </em>
          </div>
        ) : null}
        <div className="row">
          <button className="wp-icon-btn" aria-label="Previous track" onClick={() => run(api.musicPrevious)} disabled={!s?.playing}>
            ⏮
          </button>
          {s?.playing && !s.paused ? (
            <button className="btn" onClick={() => run(() => api.musicPause(true))}>
              Pause
            </button>
          ) : (
            <button className="btn" onClick={() => run(() => (s?.paused ? api.musicPause(false) : api.musicPlay(null)))}>
              Play
            </button>
          )}
          <button className="wp-icon-btn" aria-label="Next track" onClick={() => run(api.musicNext)} disabled={!s?.playing}>
            ⏭
          </button>
          <button className="btn-quiet" onClick={() => run(api.musicStop)} disabled={!s?.playing}>
            Fade out
          </button>
          {s?.ducked && (
            <button className="btn-quiet" onClick={() => run(api.musicUnduck)}>
              Raise music
            </button>
          )}
        </div>
        <label className="music-vol">
          <span>Volume</span>
          <input type="range" min={0} max={100} value={Math.round((s?.volume ?? 0.8) * 100)} onChange={(e) => run(() => api.musicVolume(Number(e.target.value) / 100))} />
        </label>
        {(problem || s?.problem) && <p className="form-error">{problem ?? s?.problem}</p>}
      </div>

      <h3 className="cue-h">Playlist</h3>
      <ul className="menu-list">
        {ids.map((id, i) => (
          <li key={id} className={s?.index === i ? "is-current" : ""}>
            <button className="menu-item" onClick={() => run(() => api.musicPlay(i))} title="Play this now">
              <span>
                {i + 1}. {byId(id)?.title ?? "Missing file"}
              </span>
              <span className="menu-item__meta wp-num">{mmss(byId(id)?.duration_ms ?? null)}</span>
              <span className="menu-item__meta">{s?.index === i ? "Playing" : ""}</span>
            </button>
            <button className="icon-btn" aria-label="Move up" onClick={() => move(i, -1)}>
              ↑
            </button>
            <button className="icon-btn" aria-label="Remove from playlist" onClick={() => setIds(ids.filter((x) => x !== id))}>
              ×
            </button>
          </li>
        ))}
        {!ids.length && <p className="empty">Add sound files below. Import them first under Library, Media, if the list is empty.</p>}
      </ul>
      {outside.length > 0 && (
        <>
          <h3 className="cue-h">Sound files in the media library</h3>
          <ul className="menu-list">
            {outside.map((a) => (
              <li key={a.id}>
                <button className="menu-item" onClick={() => setIds([...ids, a.id])}>
                  <span>
                    <Icon name="plus" /> {a.title}
                  </span>
                  <span className="menu-item__meta wp-num">{mmss(a.duration_ms)}</span>
                  <span className="menu-item__meta">Add</span>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}

      {rules && (
        <div className="form">
          <label>
            <span>Song or reading goes live</span>
            <select value={rules.on_live} onChange={(e) => setRules({ on_live: e.target.value as MusicRules["on_live"] })}>
              <option value="fade_out">Fade the music out</option>
              <option value="duck">Play it quietly under the words</option>
              <option value="keep">Keep playing as it is</option>
            </select>
          </label>
          <label className="check">
            <input type="checkbox" checked={rules.with_announcements} onChange={(e) => setRules({ with_announcements: e.target.checked })} />
            <span>Start the music when announcements go live</span>
          </label>
          <label>
            <span>Crossfade</span>
            <select value={rules.crossfade_ms} onChange={(e) => setRules({ crossfade_ms: Number(e.target.value) })}>
              {[0, 2000, 4000, 6000, 8000].map((n) => (
                <option key={n} value={n}>
                  {n ? `${n / 1000} seconds` : "None"}
                </option>
              ))}
            </select>
          </label>
          <label className="check">
            <input type="checkbox" checked={rules.looping} onChange={(e) => setRules({ looping: e.target.checked })} />
            <span>Start the playlist again after the last track</span>
          </label>
        </div>
      )}
      <div className="row row--end">
        <button className="btn-quiet" onClick={onClose}>
          Done
        </button>
      </div>
    </div>
  );
}
