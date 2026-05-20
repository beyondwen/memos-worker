PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS system_setting (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS instance_setting (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS "user" (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  row_status TEXT NOT NULL DEFAULT 'NORMAL' CHECK(row_status IN ('NORMAL', 'ARCHIVED')),
  username TEXT UNIQUE NOT NULL,
  role TEXT NOT NULL DEFAULT 'USER' CHECK(role IN ('ADMIN', 'USER')),
  email TEXT NOT NULL DEFAULT '',
  nickname TEXT NOT NULL DEFAULT '',
  password_hash TEXT NOT NULL DEFAULT '',
  avatar_url TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_user_username ON "user"(username);
CREATE INDEX IF NOT EXISTS idx_user_row_status ON "user"(row_status);

CREATE TABLE IF NOT EXISTS user_setting (
  user_id INTEGER NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (user_id, key),
  FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_session (
  id TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL,
  refresh_token_hash TEXT NOT NULL UNIQUE,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  last_used_ts INTEGER,
  expires_ts INTEGER NOT NULL,
  row_status TEXT NOT NULL DEFAULT 'NORMAL' CHECK(row_status IN ('NORMAL', 'REVOKED')),
  user_agent TEXT NOT NULL DEFAULT '',
  ip_address TEXT NOT NULL DEFAULT '',
  FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_user_session_user_id ON user_session(user_id);
CREATE INDEX IF NOT EXISTS idx_user_session_expires_ts ON user_session(expires_ts);

CREATE TABLE IF NOT EXISTS user_access_token (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  last_used_ts INTEGER,
  expires_ts INTEGER,
  row_status TEXT NOT NULL DEFAULT 'NORMAL' CHECK(row_status IN ('NORMAL', 'REVOKED')),
  FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_user_access_token_user_id ON user_access_token(user_id);
CREATE INDEX IF NOT EXISTS idx_user_access_token_prefix ON user_access_token(token_prefix);

CREATE TABLE IF NOT EXISTS user_identity (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL,
  provider TEXT NOT NULL,
  extern_uid TEXT NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  UNIQUE (provider, extern_uid),
  UNIQUE (user_id, provider),
  FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS identity_provider (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  type TEXT NOT NULL,
  identifier_filter TEXT NOT NULL DEFAULT '',
  config TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS memo (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  uid TEXT UNIQUE NOT NULL,
  creator_id INTEGER NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  row_status TEXT NOT NULL DEFAULT 'NORMAL' CHECK(row_status IN ('NORMAL', 'ARCHIVED')),
  content TEXT NOT NULL DEFAULT '',
  visibility TEXT NOT NULL DEFAULT 'PRIVATE' CHECK(visibility IN ('PUBLIC', 'PROTECTED', 'PRIVATE')),
  pinned INTEGER NOT NULL DEFAULT 0,
  payload TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_memo_creator_id ON memo(creator_id);
CREATE INDEX IF NOT EXISTS idx_memo_visibility ON memo(visibility);
CREATE INDEX IF NOT EXISTS idx_memo_row_status ON memo(row_status);
CREATE INDEX IF NOT EXISTS idx_memo_created_ts ON memo(created_ts);
CREATE INDEX IF NOT EXISTS idx_memo_updated_ts ON memo(updated_ts);
CREATE INDEX IF NOT EXISTS idx_memo_pinned ON memo(pinned);

CREATE TABLE IF NOT EXISTS memo_relation (
  memo_id INTEGER NOT NULL,
  related_memo_id INTEGER NOT NULL,
  type TEXT NOT NULL CHECK(type IN ('REFERENCE', 'COMMENT')),
  PRIMARY KEY (memo_id, related_memo_id, type),
  FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE CASCADE,
  FOREIGN KEY (related_memo_id) REFERENCES memo(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_memo_relation_related ON memo_relation(related_memo_id);

CREATE TABLE IF NOT EXISTS attachment (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  uid TEXT UNIQUE NOT NULL,
  creator_id INTEGER NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  filename TEXT NOT NULL,
  blob BLOB,
  type TEXT NOT NULL DEFAULT '',
  size INTEGER NOT NULL DEFAULT 0,
  memo_id INTEGER,
  storage_type TEXT NOT NULL DEFAULT 'S3' CHECK(storage_type IN ('DATABASE', 'LOCAL', 'S3')),
  reference TEXT NOT NULL DEFAULT '',
  payload TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE,
  FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_attachment_creator_id ON attachment(creator_id);
CREATE INDEX IF NOT EXISTS idx_attachment_memo_id ON attachment(memo_id);

CREATE TABLE IF NOT EXISTS reaction (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  creator_id INTEGER NOT NULL,
  content_type TEXT NOT NULL DEFAULT 'MEMO' CHECK(content_type IN ('MEMO')),
  content_id INTEGER NOT NULL,
  reaction_type TEXT NOT NULL,
  UNIQUE (creator_id, content_type, content_id, reaction_type),
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_reaction_content ON reaction(content_type, content_id);

CREATE TABLE IF NOT EXISTS memo_share (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  uid TEXT UNIQUE NOT NULL,
  memo_id INTEGER NOT NULL,
  creator_id INTEGER NOT NULL,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  expires_ts INTEGER,
  FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE CASCADE,
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_memo_share_uid ON memo_share(uid);
CREATE INDEX IF NOT EXISTS idx_memo_share_expires_ts ON memo_share(expires_ts);

CREATE TABLE IF NOT EXISTS inbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  sender_id INTEGER,
  receiver_id INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'UNREAD' CHECK(status IN ('UNREAD', 'READ')),
  message TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (sender_id) REFERENCES "user"(id) ON DELETE SET NULL,
  FOREIGN KEY (receiver_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_inbox_receiver ON inbox(receiver_id, status);

CREATE TABLE IF NOT EXISTS webhook (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  row_status TEXT NOT NULL DEFAULT 'NORMAL',
  creator_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  url TEXT NOT NULL,
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS shortcut (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  creator_id INTEGER NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  payload TEXT NOT NULL DEFAULT '{}',
  row_status TEXT NOT NULL DEFAULT 'NORMAL',
  created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
  FOREIGN KEY (creator_id) REFERENCES "user"(id) ON DELETE CASCADE
);
