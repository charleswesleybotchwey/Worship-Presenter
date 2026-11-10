-- "My collections": song folders the operator makes (EasyWorship's My Collections).
CREATE TABLE my_collection (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE
);
CREATE TABLE my_collection_item (
  collection_id INTEGER NOT NULL REFERENCES my_collection(id) ON DELETE CASCADE,
  song_id       INTEGER NOT NULL REFERENCES song(id) ON DELETE CASCADE,
  idx           INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (collection_id, song_id)
);
