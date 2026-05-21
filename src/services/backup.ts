import type { Env } from "../types";
import { unixNow } from "../utils";

export function buildBackupObjectKey(date = new Date()): string {
  const stamp = date.toISOString().replace(/[:.]/g, "-");
  return `backups/memos-${stamp}.json`;
}

export function backupRetentionCutoff(nowSeconds: number, retentionDays: number): number {
  return nowSeconds - retentionDays * 24 * 60 * 60;
}

export async function createBackup(env: Env): Promise<{ key: string; size: number }> {
  const key = buildBackupObjectKey();
  const body = JSON.stringify(await buildBackupPayload(env), null, 2);
  await env.MEMOS_BUCKET.put(key, body, {
    httpMetadata: { contentType: "application/json" },
    customMetadata: { createdTs: String(unixNow()) }
  });
  return { key, size: new TextEncoder().encode(body).byteLength };
}

export async function createBackupResponse(env: Env): Promise<Response> {
  const backup = await createBackup(env);
  return Response.json({ backup }, { status: 201 });
}

async function buildBackupPayload(env: Env): Promise<Record<string, unknown>> {
  const users = await env.DB.prepare(`
    SELECT id, created_ts, updated_ts, row_status, username, role, email, nickname, avatar_url, description
    FROM "user"
    ORDER BY id
  `).all();
  const memos = await env.DB.prepare("SELECT * FROM memo ORDER BY created_ts, id").all();
  const attachments = await env.DB.prepare(`
    SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload
    FROM attachment
    ORDER BY created_ts, id
  `).all();
  const relations = await env.DB.prepare("SELECT * FROM memo_relation ORDER BY memo_id, related_memo_id").all();

  return {
    exportedAt: new Date().toISOString(),
    users: users.results,
    memos: memos.results,
    attachments: attachments.results,
    relations: relations.results,
  };
}
