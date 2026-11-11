-- Rules that choose a theme automatically (first match wins; an item's own theme and
-- an output's theme still come first). match: hymnal (code, e.g. MHB), language (code,
-- e.g. fat), bilingual (yes/no), interval (service interval name), version (Bible code).
CREATE TABLE theme_rule (
  id       INTEGER PRIMARY KEY,
  position INTEGER NOT NULL,
  kind     TEXT NOT NULL CHECK (kind IN ('song', 'scripture')),
  match    TEXT NOT NULL CHECK (match IN ('hymnal', 'language', 'bilingual', 'interval', 'version')),
  value    TEXT NOT NULL,
  theme_id INTEGER NOT NULL REFERENCES theme(id) ON DELETE CASCADE
);
