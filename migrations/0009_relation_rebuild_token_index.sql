CREATE TABLE IF NOT EXISTS relation_rebuild_candidate_token (
  task_id TEXT NOT NULL,
  token TEXT NOT NULL,
  memo_id INTEGER NOT NULL,
  PRIMARY KEY (task_id, token, memo_id)
);

CREATE INDEX IF NOT EXISTS idx_relation_rebuild_candidate_token_memo
  ON relation_rebuild_candidate_token(task_id, memo_id);
