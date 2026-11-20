-- Smart song collections (EasyWorship's collection rules): a collection with rules lists
-- the songs that match them (kept up to date); without rules it holds the songs put in it.
ALTER TABLE my_collection ADD COLUMN rules TEXT;
