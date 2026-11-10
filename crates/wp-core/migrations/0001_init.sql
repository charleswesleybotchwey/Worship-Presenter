-- Worship Presenter schema v1 (spec section 5).
-- Lyrics are stored as editable source text (song_lyric) and compiled into
-- song_section + song_segment, which is what the slide splitter reads.

PRAGMA foreign_keys = ON;

CREATE TABLE collection (            -- provenance for every imported song set or Bible
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('songs','bible')),
  name          TEXT NOT NULL,
  source_format TEXT NOT NULL,       -- ew-songs, ewb, bibleshow, openlyrics, manual ...
  source_file   TEXT,
  publisher     TEXT,
  licence_note  TEXT,
  transferable  INTEGER NOT NULL DEFAULT 1,   -- 0 = warn before export/backup transfer
  imported_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE import_log (
  id            INTEGER PRIMARY KEY,
  collection_id INTEGER REFERENCES collection(id) ON DELETE CASCADE,
  level         TEXT NOT NULL CHECK (level IN ('info','fixed','skipped','error')),
  item          TEXT,
  message       TEXT NOT NULL
);

CREATE TABLE hymnal (
  id   INTEGER PRIMARY KEY,
  code TEXT NOT NULL UNIQUE,           -- PH, PAN, MHB, NCAN, GMHB ...
  name TEXT NOT NULL
);

CREATE TABLE song (
  id               INTEGER PRIMARY KEY,
  collection_id    INTEGER REFERENCES collection(id) ON DELETE SET NULL,
  title            TEXT NOT NULL,
  author           TEXT,
  copyright        TEXT,
  ccli             TEXT,
  song_key         TEXT,
  tempo            TEXT,
  default_sequence TEXT,               -- space-separated section labels, e.g. "V1 C V2 C"
  default_theme_id INTEGER,
  lines_per_slide  INTEGER CHECK (lines_per_slide BETWEEN 1 AND 8),
  notes            TEXT,
  source_ref       TEXT,               -- original id in the source database
  created_at       TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE song_number (
  song_id   INTEGER NOT NULL REFERENCES song(id) ON DELETE CASCADE,
  hymnal_id INTEGER NOT NULL REFERENCES hymnal(id) ON DELETE CASCADE,
  number    TEXT NOT NULL,             -- text so 12a and 12b are possible
  PRIMARY KEY (hymnal_id, number, song_id)
);
CREATE INDEX song_number_song ON song_number(song_id);

CREATE TABLE song_lyric (
  id      INTEGER PRIMARY KEY,
  song_id INTEGER NOT NULL REFERENCES song(id) ON DELETE CASCADE,
  lang    TEXT NOT NULL,               -- BCP-47: en, fat, tw, gaa, ee ...
  body    TEXT NOT NULL,               -- [V1] / [C] headers, blank line = slide break
  version INTEGER NOT NULL DEFAULT 1,
  UNIQUE (song_id, lang)
);

CREATE TABLE song_section (
  id      INTEGER PRIMARY KEY,
  song_id INTEGER NOT NULL REFERENCES song(id) ON DELETE CASCADE,
  label   TEXT NOT NULL,               -- V1, C, B, PC, T, E, TITLE
  idx     INTEGER NOT NULL,
  UNIQUE (song_id, idx)
);

CREATE TABLE song_segment (
  id         INTEGER PRIMARY KEY,
  section_id INTEGER NOT NULL REFERENCES song_section(id) ON DELETE CASCADE,
  lang       TEXT NOT NULL,
  idx        INTEGER NOT NULL,         -- line order within the section for that language
  text       TEXT NOT NULL,
  align_key  INTEGER NOT NULL,         -- segments sharing a key are projected together
  UNIQUE (section_id, lang, idx)
);
CREATE INDEX song_segment_section ON song_segment(section_id, align_key);

CREATE VIRTUAL TABLE song_fts USING fts5(
  title, author, lyrics, numbers,
  content='', contentless_delete=1, tokenize = "unicode61 remove_diacritics 2"
);

CREATE TABLE bible_version (
  id            INTEGER PRIMARY KEY,
  collection_id INTEGER REFERENCES collection(id) ON DELETE CASCADE,
  code          TEXT NOT NULL UNIQUE,
  name          TEXT NOT NULL,
  lang          TEXT,
  copyright     TEXT
);

CREATE TABLE bible_book (
  version_id INTEGER NOT NULL REFERENCES bible_version(id) ON DELETE CASCADE,
  book_no    INTEGER NOT NULL,         -- 1-66 canon, 67+ deuterocanon in source order
  name       TEXT NOT NULL,
  abbrev     TEXT,
  aliases    TEXT,                     -- '|' separated, matched case- and accent-insensitively
  chapters   INTEGER NOT NULL,
  PRIMARY KEY (version_id, book_no)
);

CREATE TABLE bible_verse (
  version_id INTEGER NOT NULL REFERENCES bible_version(id) ON DELETE CASCADE,
  book_no    INTEGER NOT NULL,
  chapter    INTEGER NOT NULL,
  verse      INTEGER NOT NULL,
  text       TEXT NOT NULL,            -- plain text for projection and search
  marked     TEXT,                     -- light markup kept from source: <r>words of Jesus</r>, <p> paragraph start
  heading    TEXT,                     -- section title shown before this verse, if any
  PRIMARY KEY (version_id, book_no, chapter, verse)
) WITHOUT ROWID;

CREATE VIRTUAL TABLE bible_fts USING fts5(
  text, version_id UNINDEXED, book_no UNINDEXED, chapter UNINDEXED, verse UNINDEXED,
  tokenize = "unicode61 remove_diacritics 2"
);

CREATE TABLE media (
  id          INTEGER PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('image','video','audio','loop')),
  path        TEXT NOT NULL,           -- relative to the profile's media/ folder
  title       TEXT,
  duration_ms INTEGER,
  width       INTEGER,
  height      INTEGER,
  thumb_path  TEXT,
  tags        TEXT,
  folder      TEXT
);

CREATE TABLE theme (
  id             INTEGER PRIMARY KEY,
  name           TEXT NOT NULL,
  kind           TEXT NOT NULL CHECK (kind IN ('song','scripture','announcement','lower_third','foldback')),
  schema_version INTEGER NOT NULL DEFAULT 1,
  json           TEXT NOT NULL         -- layers, animations, background, transition
);

CREATE TABLE presentation (
  id          INTEGER PRIMARY KEY,
  title       TEXT NOT NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('builtin','pptx','pdf')),
  source_path TEXT
);

CREATE TABLE presentation_slide (
  presentation_id INTEGER NOT NULL REFERENCES presentation(id) ON DELETE CASCADE,
  idx             INTEGER NOT NULL,
  json            TEXT,
  image_path      TEXT,
  duration_ms     INTEGER,
  PRIMARY KEY (presentation_id, idx)
);

CREATE TABLE schedule (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,
  service_date TEXT,
  template     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE schedule_item (
  id            INTEGER PRIMARY KEY,
  schedule_id   INTEGER NOT NULL REFERENCES schedule(id) ON DELETE CASCADE,
  idx           INTEGER NOT NULL,
  kind          TEXT NOT NULL CHECK (kind IN ('song','scripture','media','presentation','timer','blank','alert')),
  ref_id        INTEGER,
  ref_text      TEXT,                  -- scripture reference, e.g. "JHN 3:16-18|KJV,TW"
  override_json TEXT,                  -- theme, background, sequence, lines, lang_mode, custom edits
  notes         TEXT
);
CREATE INDEX schedule_item_order ON schedule_item(schedule_id, idx);

CREATE TABLE alert_preset (id INTEGER PRIMARY KEY, text TEXT NOT NULL, position TEXT, style_json TEXT);
CREATE TABLE timer_preset (id INTEGER PRIMARY KEY, name TEXT NOT NULL, mode TEXT NOT NULL CHECK (mode IN ('countdown','to_time')), value TEXT NOT NULL);
CREATE TABLE usage_log    (song_id INTEGER REFERENCES song(id) ON DELETE SET NULL, used_at TEXT NOT NULL DEFAULT (datetime('now')), schedule_id INTEGER);
CREATE TABLE setting      (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE output (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,
  role         TEXT NOT NULL CHECK (role IN ('main','foldback','alternate','ndi','stage_web')),
  display_name TEXT,                   -- bound display, NULL = not bound (NDI and stage web need none)
  bounds_json  TEXT,
  theme_id     INTEGER REFERENCES theme(id) ON DELETE SET NULL,
  logo_path    TEXT,
  on_logo      TEXT NOT NULL DEFAULT 'logo'  CHECK (on_logo  IN ('logo','blank','transparent','ignore')),
  on_blank     TEXT NOT NULL DEFAULT 'blank' CHECK (on_blank IN ('blank','transparent','ignore')),
  enabled      INTEGER NOT NULL DEFAULT 1
);

INSERT INTO output (name, role, on_logo, on_blank) VALUES
  ('Main', 'main', 'logo', 'blank'),
  ('Lower third', 'alternate', 'transparent', 'transparent');
INSERT INTO setting (key, value) VALUES
  ('lines_per_slide', '2'),
  ('project_on_restart', 'false'),
  ('low_spec_mode', 'false');
