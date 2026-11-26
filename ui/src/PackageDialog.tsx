import { useEffect, useState } from "react";
import { api, pickPackageOpen, pickPackageSave, pickSongExport, type PackageReport, type PackageReview } from "./api";

// The app's own package (.wpp): songs in every language, themes, Bibles, props and macros.

export function PackageDialog({ mode, onClose }: { mode: "export" | "import"; onClose: () => void }) {
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog pkg" role="dialog" aria-label={mode === "export" ? "Export a package" : "Import a package"}>
        <header className="wp-dialog__head">
          <h2>{mode === "export" ? "Export a Worship Presenter package" : "Import a Worship Presenter package"}</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        {mode === "export" ? <ExportPane /> : <ImportPane />}
      </div>
    </div>
  );
}

function ExportPane() {
  const [songs, setSongs] = useState(true);
  const [themes, setThemes] = useState(true);
  const [props, setProps] = useState(true);
  const [bibles, setBibles] = useState<{ code: string; name: string }[]>([]);
  const [chosen, setChosen] = useState<string[]>([]);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  useEffect(() => {
    api.versions().then((v) => setBibles(v.map((x) => ({ code: x.code, name: x.name })))).catch(() => {});
  }, []);
  const go = async () => {
    const path = await pickPackageSave();
    if (!path) return;
    try {
      const n = await api.packageExport({ all_songs: songs, song_ids: [], themes, bibles: chosen, props_and_macros: props }, path);
      setMsg({ text: `Saved (${(n / 1024 / 1024).toFixed(1)} MB). Open it on another computer with File → Import a Worship Presenter package.` });
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
  };
  return (
    <div className="pkg__body">
      <p className="wp-note">One file with everything below, every language of every song included.</p>
      <label className="wp-check">
        <input type="checkbox" checked={songs} onChange={(e) => setSongs(e.target.checked)} /> All songs (every language, hymn numbers, play order, key, collections)
      </label>
      <label className="wp-check">
        <input type="checkbox" checked={themes} onChange={(e) => setThemes(e.target.checked)} /> All themes, with their pictures and videos
      </label>
      <label className="wp-check">
        <input type="checkbox" checked={props} onChange={(e) => setProps(e.target.checked)} /> Props and macros
      </label>
      {bibles.length > 0 && (
        <fieldset className="pkg__bibles">
          <legend>Bibles</legend>
          {bibles.map((b) => (
            <label key={b.code} className="wp-check">
              <input type="checkbox" checked={chosen.includes(b.code)} onChange={(e) => setChosen((c) => (e.target.checked ? [...c, b.code] : c.filter((x) => x !== b.code)))} /> {b.code} · {b.name}
            </label>
          ))}
          <p className="wp-note">Share a Bible only where its licence allows it.</p>
        </fieldset>
      )}
      {msg && <p className={msg.bad ? "form-error" : "wp-note"}>{msg.text}</p>}
      <div className="ps__foot">
        <button className="wp-btn wp-btn--primary" disabled={!songs && !themes && !props && !chosen.length} onClick={() => void go()}>
          Export…
        </button>
      </div>
    </div>
  );
}

function ImportPane() {
  const [path, setPath] = useState<string | null>(null);
  const [rev, setRev] = useState<PackageReview | null>(null);
  const [replace, setReplace] = useState(false);
  const [done, setDone] = useState<PackageReport | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const choose = async () => {
    const p = await pickPackageOpen();
    if (!p) return;
    setPath(p);
    setDone(null);
    setMsg(null);
    try {
      setRev(await api.packageReview(p));
    } catch (e) {
      setRev(null);
      setMsg(String(e instanceof Error ? e.message : e));
    }
  };
  const go = async () => {
    if (!path) return;
    try {
      setDone(await api.packageImport(path, replace));
    } catch (e) {
      setMsg(String(e instanceof Error ? e.message : e));
    }
  };
  return (
    <div className="pkg__body">
      {!rev && (
        <>
          <p className="wp-note">A .wpp file made with File → Export a Worship Presenter package.</p>
          <button className="wp-btn wp-btn--primary" onClick={() => void choose()}>
            Choose a package…
          </button>
        </>
      )}
      {rev && !done && (
        <>
          <ul className="pkg__list">
            <li>
              <strong>{rev.manifest.songs.toLocaleString()} songs</strong>: {rev.new_songs.toLocaleString()} new, {rev.duplicate_songs.toLocaleString()} already here
              {rev.multilingual_songs ? `; ${rev.multilingual_songs.toLocaleString()} in more than one language` : ""}
            </li>
            <li>
              <strong>{rev.manifest.themes} themes</strong>: {rev.new_themes} new
            </li>
            {rev.manifest.bibles.length > 0 && (
              <li>
                <strong>Bibles</strong>: {rev.new_bibles.length ? `${rev.new_bibles.join(", ")} new` : "none new"}
                {rev.present_bibles.length ? `; ${rev.present_bibles.join(", ")} already here` : ""}
              </li>
            )}
            {rev.manifest.props + rev.manifest.macros > 0 && (
              <li>
                <strong>{rev.manifest.props} props, {rev.manifest.macros} macros</strong> (ones with the same name are kept as they are)
              </li>
            )}
          </ul>
          {rev.sample_titles.length > 0 && <p className="wp-note">For example: {rev.sample_titles.join(" · ")}</p>}
          {rev.duplicate_songs > 0 && (
            <label className="wp-check">
              <input type="checkbox" checked={replace} onChange={(e) => setReplace(e.target.checked)} /> Replace the {rev.duplicate_songs.toLocaleString()} songs already here with the package's (otherwise they are left as they are)
            </label>
          )}
          <p className="wp-note">A backup is made first.</p>
          <div className="ps__foot">
            <button className="wp-btn" onClick={() => void choose()}>
              Another package…
            </button>
            <button className="wp-btn wp-btn--primary" onClick={() => void go()}>
              Import
            </button>
          </div>
        </>
      )}
      {done && (
        <p className="pkg__done" role="status">
          Imported: {done.songs_added.toLocaleString()} songs added
          {done.songs_replaced ? `, ${done.songs_replaced.toLocaleString()} replaced` : ""}
          {done.songs_skipped ? `, ${done.songs_skipped.toLocaleString()} left as they were` : ""}; {done.themes_added} themes
          {done.bibles_added.length ? `; Bibles ${done.bibles_added.join(", ")}` : ""}
          {done.props_added + done.macros_added ? `; ${done.props_added} props, ${done.macros_added} macros` : ""}.
        </p>
      )}
      {msg && <p className="form-error">{msg}</p>}
    </div>
  );
}

/** Songs out, one or many (as ProPresenter): Worship Presenter, OpenLyrics or text. */
export function SongExportDialog({ ids, onClose }: { ids: number[]; onClose: () => void }) {
  const [format, setFormat] = useState<"wpp" | "openlyrics" | "text">("wpp");
  const [each, setEach] = useState(false);
  const [title, setTitle] = useState("Songs");
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  const many = ids.length > 1;
  // OpenLyrics holds one song a file
  const perSong = many && (each || format === "openlyrics");
  useEffect(() => {
    if (ids.length === 1) api.getSong(ids[0]).then((d) => setTitle(d.title)).catch(() => {});
  }, [ids]);
  const go = async () => {
    const day = new Date().toISOString().slice(0, 10);
    const path = await pickSongExport(format, many ? `${ids.length} songs ${day}` : title.replace(/[<>:"/\\|?*]/g, "_"), perSong);
    if (!path) return;
    try {
      const n = await api.songsExport(ids, format, path, perSong);
      setMsg({ text: perSong ? `${n.toLocaleString()} files saved in the folder.` : `Saved: ${n.toLocaleString()} song${n === 1 ? "" : "s"}.` });
    } catch (e) {
      setMsg({ text: String(e instanceof Error ? e.message : e), bad: true });
    }
  };
  const FORMATS: ["wpp" | "openlyrics" | "text", string, string][] = [
    ["wpp", "Worship Presenter (.wpp)", "Every language, numbers, play order and key: for another Worship Presenter."],
    ["openlyrics", "OpenLyrics (.xml)", "The open standard OpenLP, FreeShow and others read; every language kept."],
    ["text", "Plain text (.txt)", "The words, each section with its languages: for printing, WhatsApp or the choir."],
  ];
  return (
    <div className="wp-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="wp-dialog pkg" role="dialog" aria-label="Export songs">
        <header className="wp-dialog__head">
          <h2>{many ? `Export ${ids.length.toLocaleString()} songs` : `Export “${title}”`}</h2>
          <button className="wp-btn wp-btn--quiet" onClick={onClose}>
            Close
          </button>
        </header>
        <div className="pkg__body">
          {FORMATS.map(([f, name, hint]) => (
            <label key={f} className="pkg__format">
              <input type="radio" name="song-format" checked={format === f} onChange={() => setFormat(f)} />
              <span>
                <strong>{name}</strong>
                <small className="wp-note">{hint}</small>
              </span>
            </label>
          ))}
          {many && (
            <label className="wp-check">
              <input type="checkbox" checked={perSong} disabled={format === "openlyrics"} onChange={(e) => setEach(e.target.checked)} /> One file per song (into a folder)
              {format === "openlyrics" ? " — always, for OpenLyrics" : ""}
            </label>
          )}
          {msg && <p className={msg.bad ? "form-error" : "pkg__done"}>{msg.text}</p>}
          <div className="ps__foot">
            <button className="wp-btn wp-btn--primary" onClick={() => void go()}>
              Export…
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
