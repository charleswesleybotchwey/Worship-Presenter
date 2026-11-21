-- Macros (as ProPresenter): one button that runs several actions in order; and slide
-- actions: a macro run when a particular slide goes live.
CREATE TABLE macro (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL,
  json  TEXT NOT NULL
);
CREATE TABLE slide_action (
  item_kind TEXT NOT NULL,          -- song | announcement
  item_id   INTEGER NOT NULL,
  slide_key TEXT NOT NULL,          -- a song's section ("C") or a deck slide's number
  macro_id  INTEGER NOT NULL REFERENCES macro(id) ON DELETE CASCADE,
  PRIMARY KEY (item_kind, item_id, slide_key)
);
