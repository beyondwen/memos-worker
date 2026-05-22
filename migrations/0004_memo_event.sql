CREATE TABLE IF NOT EXISTS memo_event (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  event_type TEXT NOT NULL,
  name TEXT NOT NULL,
  visibility TEXT NOT NULL DEFAULT 'PRIVATE',
  creator_id INTEGER NOT NULL,
  payload TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_memo_event_created ON memo_event(created_ts, id);
CREATE INDEX IF NOT EXISTS idx_memo_event_visibility ON memo_event(visibility, creator_id, id);
