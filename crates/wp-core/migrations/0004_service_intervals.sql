-- EasyWorship "Service Intervals": named service times; song use is recorded with the
-- interval it happened in, for CCLI reports.
CREATE TABLE service_interval (
  id       INTEGER PRIMARY KEY,
  name     TEXT NOT NULL,
  weekday  INTEGER NOT NULL CHECK (weekday BETWEEN -1 AND 6),  -- 0 Sunday … 6 Saturday, -1 every day
  start    TEXT NOT NULL,  -- HH:MM
  end      TEXT NOT NULL   -- HH:MM
);
ALTER TABLE usage_log ADD COLUMN interval_name TEXT;
INSERT INTO output (name, role, on_logo, on_blank) SELECT 'Stage', 'foldback', 'blank', 'blank' WHERE NOT EXISTS (SELECT 1 FROM output WHERE role = 'foldback');
