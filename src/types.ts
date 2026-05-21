export interface Env {
  DB: D1Database;
  MEMOS_BUCKET: R2Bucket;
  SSE_HUB: DurableObjectNamespace;
  ASSETS?: { fetch: (input: RequestInfo, init?: RequestInit) => Promise<Response> };
  SERVER_SECRET: string;
  ENVIRONMENT?: string;
  AI_API_KEY?: string;
  AI_BASE_URL?: string;
  AI_MODEL?: string;
}

export type Role = "ADMIN" | "USER";
export type RowStatus = "NORMAL" | "ARCHIVED";
export type Visibility = "PUBLIC" | "PROTECTED" | "PRIVATE";

export interface DbUser {
  id: number;
  created_ts: number;
  updated_ts: number;
  row_status: RowStatus;
  username: string;
  role: Role;
  email: string;
  nickname: string;
  password_hash: string;
  avatar_url: string;
  description: string;
}

export interface DbMemo {
  id: number;
  uid: string;
  creator_id: number;
  created_ts: number;
  updated_ts: number;
  row_status: RowStatus;
  content: string;
  visibility: Visibility;
  pinned: number;
  payload: string;
  creator_username?: string;
  creator_nickname?: string;
}

export interface DbAttachment {
  id: number;
  uid: string;
  creator_id: number;
  created_ts: number;
  updated_ts: number;
  filename: string;
  type: string;
  size: number;
  memo_id: number | null;
  storage_type: "S3" | "DATABASE" | "LOCAL";
  reference: string;
  payload: string;
  memo_visibility?: Visibility | null;
  memo_creator_id?: number | null;
}

export interface DbUserAccessToken {
  id: number;
  user_id: number;
  name: string;
  token_prefix: string;
  token_hash: string;
  created_ts: number;
  updated_ts: number;
  last_used_ts: number | null;
  expires_ts: number | null;
  row_status: RowStatus;
}

export interface DbUserSetting {
  user_id: number;
  key: string;
  value: string;
}

export interface Claims {
  type: "access" | "refresh" | "sse";
  role: Role;
  status: RowStatus;
  username: string;
  iss: "memos-worker";
  aud: string[];
  sub: string;
  tid?: string;
  iat: number;
  exp: number;
}

export interface Viewer {
  id: number;
  username: string;
  role: Role;
  rowStatus: RowStatus;
}

export interface SseEvent {
  id: string;
  type: "memo.created" | "memo.updated" | "memo.archived" | "memo.restored" | "memo.deleted" | "memo.bulk.updated" | "memo.comment.created" | "reaction.upserted" | "reaction.deleted";
  name: string;
  visibility: Visibility;
  creatorId: number;
}
