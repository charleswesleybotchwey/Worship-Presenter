-- Timelines (as ProPresenter): a song or deck run in time with a backing track — the
-- moments (ms into the track) each slide goes live.
CREATE TABLE timeline (
  item_kind TEXT NOT NULL,          -- song | announcement
  item_id   INTEGER NOT NULL,
  media_id  INTEGER NOT NULL,
  cues      TEXT NOT NULL,          -- JSON [[ms, slide index], …], in time order
  PRIMARY KEY (item_kind, item_id)
);
