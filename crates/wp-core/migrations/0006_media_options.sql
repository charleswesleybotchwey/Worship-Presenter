-- EasyWorship's per-media Repeating and Aspect Ratio (right-click on a video or picture).
-- repeat: 'loop' | 'once' | NULL (the default: play once, as before)
-- fit:    'fit' (whole picture, bars) | 'fill' (crop to the screen) | 'stretch' | NULL ('fit')
ALTER TABLE media ADD COLUMN repeat TEXT;
ALTER TABLE media ADD COLUMN fit TEXT;
