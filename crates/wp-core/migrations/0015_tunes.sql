-- Hymn tunes: a recording of one verse of a song's tune (media library), played once per
-- verse so a congregation can sing without an organist; the breath between verses.
CREATE TABLE song_tune (
  song_id  INTEGER PRIMARY KEY REFERENCES song(id) ON DELETE CASCADE,
  media_id INTEGER NOT NULL,
  gap_ms   INTEGER NOT NULL DEFAULT 1500
);
