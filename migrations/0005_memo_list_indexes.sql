CREATE INDEX IF NOT EXISTS idx_memo_list_home ON memo(row_status, pinned, created_ts, id);
