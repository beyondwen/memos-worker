ALTER TABLE relation_rebuild_task
  ADD COLUMN failed_attempts INTEGER NOT NULL DEFAULT 0;

ALTER TABLE relation_rebuild_task
  ADD COLUMN error TEXT NOT NULL DEFAULT '';
