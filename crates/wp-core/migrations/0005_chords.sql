-- Chords for the musicians' stage view: the lyric line with its inline chords
-- ("[G]Amazing [C]grace"); text holds the words only.
ALTER TABLE song_segment ADD COLUMN chords TEXT;
