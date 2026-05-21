CREATE TABLE IF NOT EXISTS webhook_delivery (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  webhook_id INTEGER NOT NULL,
  creator_id INTEGER NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  event TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('SUCCESS', 'FAILED')),
  status_code INTEGER,
  duration_ms INTEGER NOT NULL DEFAULT 0,
  error TEXT NOT NULL DEFAULT '',
  request_body TEXT NOT NULL DEFAULT '{}',
  response_body TEXT NOT NULL DEFAULT '',
  FOREIGN KEY (webhook_id) REFERENCES webhook(id) ON DELETE CASCADE,
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_webhook_delivery_creator ON webhook_delivery(creator_id, created_ts);
CREATE INDEX IF NOT EXISTS idx_webhook_delivery_webhook ON webhook_delivery(webhook_id, created_ts);
CREATE INDEX IF NOT EXISTS idx_webhook_delivery_status ON webhook_delivery(status, created_ts);
