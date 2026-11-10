import { useEffect, useState } from "react";
import { api, pickBackup, type BackupInfo } from "./api";

// Backups of the whole library. A restore always shows what the backup holds first,
// and saves the current library as a safety copy, so any restore can be undone.

const REASONS: Record<string, string> = {
  manual: "Made by you",
  weekly: "Weekly, automatic",
  "before-import": "Before an import",
  "before-restore": "Before a restore",
};
const size = (b: number) => (b > 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

export function BackupsDialog({ onClose, onRestored }: { onClose: () => void; onRestored: () => void }) {
  const [list, setList] = useState<BackupInfo[]>([]);
  const [folder, setFolder] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; bad?: boolean } | null>(null);

  const refresh = () => api.listBackups().then(setList).catch((e) => setMessage({ text: String(e), bad: true }));
  useEffect(() => {
    refresh();
    api.backupsFolder().then(setFolder).catch(() => {});
  }, []);

  const backupNow = async () => {
    setBusy(true);
    try {
      const b = await api.backupNow();
      setMessage({ text: `Backed up (${size(b.bytes)}). Copy the backups folder to a USB stick or cloud drive to keep a copy off this computer.` });
      refresh();
    } catch (e) {
      setMessage({ text: String(e), bad: true });
    } finally {
      setBusy(false);
    }
  };

  const restore = async (path: string) => {
    setBusy(true);
    try {
      const c = await api.inspectBackup(path);
      const ok = confirm(
        `This backup holds ${c.songs.toLocaleString()} songs, ${c.bibles} Bibles and ${c.schedules} service orders.\n\n` +
          `Restoring replaces your current library. Your current library is saved first, so you can undo this.\n\nRestore it?`,
      );
      if (!ok) return;
      const safety = await api.restoreBackup(path);
      setMessage({ text: `Restored. To undo, restore the backup taken ${safety.taken} (“Before a restore”).` });
      refresh();
      onRestored();
    } catch (e) {
      setMessage({ text: `Not restored: ${String(e instanceof Error ? e.message : e)}`, bad: true });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && !busy && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-label="Backups">
        <div className="dialog__head">
          <span className="pane__title">Library backups</span>
          <button className="btn-quiet" onClick={onClose} disabled={busy}>
            Done
          </button>
        </div>
        <div className="row row--split">
          <button className="btn" onClick={backupNow} disabled={busy}>
            Back up now
          </button>
          <button
            className="btn-quiet"
            disabled={busy}
            onClick={async () => {
              const p = await pickBackup();
              if (p) restore(p);
            }}
          >
            Restore from a file…
          </button>
        </div>
        {message && <p className={message.bad ? "form-error" : "notice"}>{message.text}</p>}
        <ul className="menu-list">
          {list.map((b) => (
            <li key={b.file}>
              <div className="menu-item menu-item--static">
                <span>{b.taken}</span>
                <span className="menu-item__meta">{REASONS[b.reason] ?? b.reason}</span>
                <span className="menu-item__meta">{size(b.bytes)}</span>
              </div>
              <button className="btn-quiet" disabled={busy} onClick={() => restore(b.path)}>
                Restore
              </button>
            </li>
          ))}
          {!list.length && <p className="empty">No backups yet. One is taken automatically each week and before every import.</p>}
        </ul>
        {folder && (
          <p className="hint hint--foot">
            Backups are kept in <span className="path">{folder}</span>. Automatic backups keep the latest 12; ones you make are never deleted.
          </p>
        )}
      </div>
    </div>
  );
}
