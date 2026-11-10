import { useEffect, useState } from "react";
import { api, type Profile } from "./api";

// One library per church or venue. Switching takes whatever is live off the screen,
// because it belongs to the other library, so it asks first during a service.

export function ProfilesDialog({ somethingLive, onClose }: { somethingLive: boolean; onClose: () => void }) {
  const [list, setList] = useState<Profile[]>([]);
  const [name, setName] = useState("");
  const [copy, setCopy] = useState(false);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameTo, setRenameTo] = useState("");
  const [message, setMessage] = useState<{ text: string; bad?: boolean } | null>(null);

  const refresh = () => api.listProfiles().then(setList).catch((e) => setMessage({ text: String(e), bad: true }));
  useEffect(() => void refresh(), []);
  const current = list.find((p) => p.active);

  const attempt = async (f: () => Promise<unknown>, ok?: string) => {
    try {
      await f();
      setMessage(ok ? { text: ok } : null);
      refresh();
      return true;
    } catch (e) {
      setMessage({ text: String(e instanceof Error ? e.message : e), bad: true });
      return false;
    }
  };

  const switchTo = async (p: Profile) => {
    if (somethingLive && !confirm(`Something is on screen now. Switching to “${p.name}” blanks the outputs. Switch?`)) return;
    if (await attempt(() => api.switchProfile(p.slug))) onClose();
  };

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-label="Profiles">
        <div className="dialog__head">
          <span className="pane__title">Profiles</span>
          <button className="btn-quiet" onClick={onClose}>
            Done
          </button>
        </div>
        <p className="hint hint--foot">Each profile has its own songs, Bibles, service orders and backups. Use one per church, branch or venue.</p>
        {message && <p className={message.bad ? "form-error" : "notice"}>{message.text}</p>}
        <ul className="menu-list">
          {list.map((p) => (
            <li key={p.slug} className={p.active ? "is-current" : ""}>
              {renaming === p.slug ? (
                <form
                  className="rename"
                  onSubmit={async (e) => {
                    e.preventDefault();
                    if (await attempt(() => api.renameProfile(p.slug, renameTo))) setRenaming(null);
                  }}
                >
                  <input value={renameTo} onChange={(e) => setRenameTo(e.target.value)} autoFocus aria-label="New name" />
                  <button className="btn" type="submit">
                    Save
                  </button>
                  <button className="btn-quiet" type="button" onClick={() => setRenaming(null)}>
                    Cancel
                  </button>
                </form>
              ) : (
                <>
                  <div className="menu-item menu-item--static">
                    <span>{p.name}</span>
                    <span className="menu-item__meta">{p.active ? "In use" : ""}</span>
                    <span className="menu-item__meta">
                      {p.songs.toLocaleString()} songs, {p.bibles} Bibles
                    </span>
                  </div>
                  {!p.active && (
                    <button className="btn-quiet" onClick={() => switchTo(p)}>
                      Switch
                    </button>
                  )}
                  <button className="btn-quiet" onClick={() => (setRenaming(p.slug), setRenameTo(p.name))}>
                    Rename
                  </button>
                  {!p.active && (
                    <button
                      className="btn-quiet btn-danger"
                      onClick={() =>
                        confirm(`Delete the profile “${p.name}” with its songs, Bibles and service orders? It is moved to a recovery folder, not erased.`) &&
                        attempt(() => api.deleteProfile(p.slug), `“${p.name}” deleted. It can be recovered from the profiles\\.deleted folder.`)
                      }
                    >
                      Delete
                    </button>
                  )}
                </>
              )}
            </li>
          ))}
        </ul>
        <form
          className="form form--inline"
          onSubmit={async (e) => {
            e.preventDefault();
            if (await attempt(() => api.createProfile(name, copy), `“${name.trim()}” created. Switch to it when you are ready.`)) setName("");
          }}
        >
          <label>
            <span>New profile</span>
            <input value={name} placeholder="e.g. Youth Service, Akwatia Branch" onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="check">
            <input type="checkbox" checked={copy} onChange={(e) => setCopy(e.target.checked)} />
            <span>Start with a copy of {current ? `“${current.name}”` : "this library"} (songs, Bibles, service orders)</span>
          </label>
          <div className="row row--end">
            <button className="btn" type="submit" disabled={!name.trim()}>
              Create
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
