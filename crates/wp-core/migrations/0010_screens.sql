-- Screens (as ProPresenter): any number of named outputs, each with a look (full slides,
-- lower third, stage display, words only, media only). The fixed roles main / alternate /
-- foldback stay as the first three screens; the check that allowed only those goes.
CREATE TABLE output_new (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,
  role         TEXT NOT NULL UNIQUE,
  display_name TEXT,
  bounds_json  TEXT,
  theme_id     INTEGER REFERENCES theme(id) ON DELETE SET NULL,
  logo_path    TEXT,
  on_logo      TEXT NOT NULL DEFAULT 'logo'  CHECK (on_logo  IN ('logo','blank','transparent','ignore')),
  on_blank     TEXT NOT NULL DEFAULT 'blank' CHECK (on_blank IN ('blank','transparent','ignore')),
  enabled      INTEGER NOT NULL DEFAULT 1
);
INSERT INTO output_new (id, name, role, display_name, bounds_json, theme_id, logo_path, on_logo, on_blank, enabled)
  SELECT min(id), name, role, display_name, bounds_json, theme_id, logo_path, on_logo, on_blank, enabled FROM output GROUP BY role;
DROP TABLE output;
ALTER TABLE output_new RENAME TO output;
INSERT INTO output (name, role, on_logo, on_blank) SELECT 'Stage', 'foldback', 'blank', 'blank' WHERE NOT EXISTS (SELECT 1 FROM output WHERE role = 'foldback');
