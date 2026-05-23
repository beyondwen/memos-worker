CREATE TABLE IF NOT EXISTS relation_rebuild_task (
  id TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL,
  created_ts INTEGER NOT NULL,
  updated_ts INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'RUNNING',
  mode TEXT NOT NULL DEFAULT 'supplement',
  cursor INTEGER NOT NULL DEFAULT 0,
  total INTEGER NOT NULL DEFAULT 0,
  processed INTEGER NOT NULL DEFAULT 0,
  created INTEGER NOT NULL DEFAULT 0,
  updated INTEGER NOT NULL DEFAULT 0,
  skipped INTEGER NOT NULL DEFAULT 0,
  source TEXT NOT NULL DEFAULT 'local',
  warnings TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_relation_rebuild_task_status
  ON relation_rebuild_task(status, updated_ts);

CREATE INDEX IF NOT EXISTS idx_relation_rebuild_task_user
  ON relation_rebuild_task(user_id, created_ts);

CREATE TABLE IF NOT EXISTS relation_rebuild_candidate (
  task_id TEXT NOT NULL,
  memo_id INTEGER NOT NULL,
  user_id INTEGER NOT NULL,
  uid TEXT NOT NULL,
  created_ts INTEGER NOT NULL,
  updated_ts INTEGER NOT NULL,
  content TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'PRIVATE',
  pinned INTEGER NOT NULL DEFAULT 0,
  payload TEXT NOT NULL DEFAULT '{}',
  PRIMARY KEY (task_id, memo_id)
);

CREATE INDEX IF NOT EXISTS idx_relation_rebuild_candidate_user
  ON relation_rebuild_candidate(task_id, user_id);
