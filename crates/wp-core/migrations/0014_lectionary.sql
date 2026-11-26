-- The church's lectionary (imported once a year): readings by date ("date:2026-11-29") or by
-- Sunday name and year ("sunday:B:advent 1", "sunday:ABC:easter day").
CREATE TABLE lectionary (
  key      TEXT PRIMARY KEY,
  occasion TEXT,
  readings TEXT NOT NULL   -- JSON list, in order
);
