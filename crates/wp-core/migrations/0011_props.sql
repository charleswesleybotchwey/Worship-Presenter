-- Props (as ProPresenter): graphics that stay on screen independently of the slides — a
-- logo bug, a "LIVE" badge, a sermon title, a clock. Shown and cleared on their own.
CREATE TABLE prop (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL,
  json  TEXT NOT NULL
);
