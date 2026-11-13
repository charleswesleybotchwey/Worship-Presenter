-- "My collections" for scripture readings and media (as EasyWorship's sidebars): named
-- lists of items kept in order. Songs have their own collections (0003).
CREATE TABLE item_collection (
  id   INTEGER PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('scripture', 'media')),
  name TEXT NOT NULL
);
CREATE TABLE item_collection_entry (
  collection_id INTEGER NOT NULL REFERENCES item_collection(id) ON DELETE CASCADE,
  position      INTEGER NOT NULL,
  item          TEXT NOT NULL   -- the program item as JSON (a reading or a media item)
);
CREATE INDEX item_collection_entry_by_collection ON item_collection_entry(collection_id, position);
