import { useEffect, useState } from "react";
import { api, type Schedule, type ScheduleSummary } from "./api";
import { Icon } from "./Icon";

// Open or start a service order. Templates (e.g. "Sunday first service") are listed
// first; a new service can start from one.

function nextSunday() {
  const d = new Date();
  d.setDate(d.getDate() + ((7 - d.getDay()) % 7 || 7));
  return d.toISOString().slice(0, 10);
}

export function niceDate(iso: string | null) {
  if (!iso) return "";
  const d = new Date(`${iso}T12:00:00`);
  return d.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
}

export function ServiceMenu({ current, onOpen, onClose }: { current: number | null; onOpen: (s: Schedule) => void; onClose: () => void }) {
  const [list, setList] = useState<ScheduleSummary[]>([]);
  const [mode, setMode] = useState<"open" | "new">("open");
  const [name, setName] = useState("Sunday service");
  const [date, setDate] = useState(nextSunday());
  const [from, setFrom] = useState<number | "">("");
  const [asTemplate, setAsTemplate] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => api.listSchedules().then(setList).catch((e) => setError(String(e)));
  useEffect(() => void refresh(), []);
  const templates = list.filter((s) => s.template);

  const create = async () => {
    try {
      const s = await api.newSchedule(name.trim() || "Service", asTemplate ? null : date, asTemplate, from === "" ? null : from);
      onOpen(s);
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="popover popover--left" role="dialog" aria-label="Service orders">
      <div className="tabs" role="tablist">
        <button role="tab" aria-selected={mode === "open"} onClick={() => setMode("open")}>
          Open
        </button>
        <button role="tab" aria-selected={mode === "new"} onClick={() => setMode("new")}>
          New
        </button>
      </div>
      {mode === "open" ? (
        <ul className="menu-list">
          {list.map((s) => (
            <li key={s.id} className={s.id === current ? "is-current" : ""}>
              <button
                className="menu-item"
                onClick={async () => {
                  try {
                    onOpen(await api.openSchedule(s.id));
                    onClose();
                  } catch (e) {
                    setError(String(e));
                  }
                }}
              >
                <span>{s.name}</span>
                <span className="menu-item__meta">{s.template ? "Template" : niceDate(s.service_date)}</span>
                <span className="menu-item__meta">{s.items === 1 ? "1 item" : `${s.items} items`}</span>
              </button>
              <button
                className="icon-btn"
                aria-label={`Delete ${s.name}`}
                title="Delete"
                onClick={async () => {
                  if (!confirm(`Delete “${s.name}”? This cannot be undone.`)) return;
                  await api.deleteSchedule(s.id);
                  refresh();
                }}
              >
                <Icon name="close" />
              </button>
            </li>
          ))}
          {!list.length && <p className="empty">No saved service orders yet. Start one under New.</p>}
        </ul>
      ) : (
        <div className="form">
          <label>
            <span>Name</span>
            <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          </label>
          <label className="check">
            <input type="checkbox" checked={asTemplate} onChange={(e) => setAsTemplate(e.target.checked)} />
            <span>Template to reuse every week (no date)</span>
          </label>
          {!asTemplate && (
            <>
              <label>
                <span>Date</span>
                <input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
              </label>
              <label>
                <span>Start from</span>
                <select value={from} onChange={(e) => setFrom(e.target.value === "" ? "" : Number(e.target.value))}>
                  <option value="">Empty</option>
                  {templates.map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.name}
                    </option>
                  ))}
                </select>
              </label>
            </>
          )}
          <div className="row row--end">
            <button className="btn-quiet" onClick={onClose}>
              Cancel
            </button>
            <button className="btn" onClick={create}>
              Create
            </button>
          </div>
        </div>
      )}
      {error && <p className="form-error">{error}</p>}
    </div>
  );
}
