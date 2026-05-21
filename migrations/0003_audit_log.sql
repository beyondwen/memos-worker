CREATE TABLE IF NOT EXISTS audit_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  actor_id INTEGER,
  action TEXT NOT NULL,
  target TEXT NOT NULL DEFAULT '',
  detail TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (actor_id) REFERENCES "user"(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_ts);
CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action, created_ts);
