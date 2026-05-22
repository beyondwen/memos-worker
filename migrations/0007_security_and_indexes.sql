CREATE TABLE IF NOT EXISTS auth_rate_limit (
  scope TEXT NOT NULL,
  actor_key TEXT NOT NULL,
  window_start_ts INTEGER NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  blocked_until_ts INTEGER NOT NULL DEFAULT 0,
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  PRIMARY KEY (scope, actor_key)
);

CREATE INDEX IF NOT EXISTS idx_auth_rate_limit_updated_ts ON auth_rate_limit(updated_ts);

CREATE TABLE IF NOT EXISTS memo_search (
  memo_id INTEGER PRIMARY KEY,
  content TEXT NOT NULL DEFAULT '',
  updated_ts INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS memo_tag (
  memo_id INTEGER NOT NULL,
  tag TEXT NOT NULL,
  PRIMARY KEY (memo_id, tag),
  FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_memo_tag_tag ON memo_tag(tag, memo_id);
CREATE INDEX IF NOT EXISTS idx_memo_search_updated_ts ON memo_search(updated_ts);

INSERT OR REPLACE INTO memo_search (memo_id, content, updated_ts)
SELECT id, content, updated_ts FROM memo;

INSERT OR IGNORE INTO memo_tag (memo_id, tag)
SELECT memo.id, TRIM(json_each.value)
FROM memo, json_each(CASE WHEN json_valid(memo.payload) THEN memo.payload ELSE '{}' END, '$.tags')
WHERE json_each.type = 'text' AND TRIM(json_each.value) != '';
