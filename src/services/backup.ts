import type { Env, Viewer } from "../types";
import { json, readJson, unixNow } from "../utils";
import { recordAudit } from "./audit";

export function buildBackupObjectKey(date = new Date()): string {
  const stamp = date.toISOString().replace(/[:.]/g, "-");
  return `backups/memos-${stamp}.json`;
}

export function backupRetentionCutoff(nowSeconds: number, retentionDays: number): number {
  return nowSeconds - retentionDays * 24 * 60 * 60;
}

export function previewBackupPayload(payload: Record<string, unknown>): {
  userCount: number;
  memoCount: number;
  attachmentCount: number;
  relationCount: number;
} {
  return {
    userCount: Array.isArray(payload.users) ? payload.users.length : 0,
    memoCount: Array.isArray(payload.memos) ? payload.memos.length : 0,
    attachmentCount: Array.isArray(payload.attachments) ? payload.attachments.length : 0,
    relationCount: Array.isArray(payload.relations) ? payload.relations.length : 0,
  };
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

export async function createBackupResponse(env: Env, viewer?: Viewer): Promise<Response> {
  const backup = await createBackup(env);
  await recordAudit(env, viewer ?? null, "backup.create", backup.key, { size: backup.size });
  return Response.json({ backup }, { status: 201 });
}

export async function listBackups(env: Env, viewer: Viewer): Promise<Response> {
  const listed = await env.MEMOS_BUCKET.list({ prefix: "backups/" });
  return json({
    backups: listed.objects
      .sort((a, b) => b.uploaded.getTime() - a.uploaded.getTime())
      .map((object) => ({
        key: object.key,
        size: object.size,
        uploaded: object.uploaded.toISOString()
      }))
  });
}

export async function downloadBackup(request: Request, env: Env): Promise<Response> {
  const key = new URL(request.url).searchParams.get("key") ?? "";
  if (!key.startsWith("backups/")) return json({ error: "Invalid backup key" }, 400);
  const object = await env.MEMOS_BUCKET.get(key);
  if (!object) return json({ error: "Backup not found" }, 404);
  return new Response(object.body, {
    headers: {
      "Content-Type": "application/json",
      "Content-Disposition": `attachment; filename="${key.split("/").pop() ?? "memos-backup.json"}"`
    }
  });
}

export async function previewBackup(request: Request, env: Env): Promise<Response> {
  const payload = await readBackupPayload(request, env);
  return json({ preview: previewBackupPayload(payload) });
}

export async function restoreBackup(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const payload = await readBackupPayload(request, env);
  const preview = previewBackupPayload(payload);

  for (const item of asArray(payload.memos)) {
    await env.DB.prepare(`
      INSERT OR REPLACE INTO memo (id, uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).bind(
      item.id, item.uid, item.creator_id, item.created_ts, item.updated_ts,
      item.row_status ?? "NORMAL", item.content ?? "", item.visibility ?? "PRIVATE",
      item.pinned ? 1 : 0, item.payload ?? "{}"
    ).run();
  }
  for (const item of asArray(payload.attachments)) {
    await env.DB.prepare(`
      INSERT OR REPLACE INTO attachment (id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).bind(
      item.id, item.uid, item.creator_id, item.created_ts, item.updated_ts,
      item.filename ?? "attachment", item.type ?? "", item.size ?? 0, item.memo_id ?? null,
      item.storage_type ?? "S3", item.reference ?? "", item.payload ?? "{}"
    ).run();
  }
  await env.DB.prepare("DELETE FROM memo_relation").run();
  for (const item of asArray(payload.relations)) {
    await env.DB.prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, ?)")
      .bind(item.memo_id, item.related_memo_id, item.type ?? "REFERENCE")
      .run();
  }

  await recordAudit(env, viewer, "backup.restore", "backup", preview);
  return json({ restored: preview });
}

async function readBackupPayload(request: Request, env: Env): Promise<Record<string, unknown>> {
  const body = await readJson<{ key?: string; payload?: Record<string, unknown> }>(request);
  if (body.payload) return body.payload;
  const key = body.key ?? "";
  if (!key.startsWith("backups/")) throw new Error("Invalid backup key");
  const object = await env.MEMOS_BUCKET.get(key);
  if (!object) throw new Error("Backup not found");
  return JSON.parse(await object.text()) as Record<string, unknown>;
}

function asArray(value: unknown): Array<Record<string, any>> {
  return Array.isArray(value) ? value as Array<Record<string, any>> : [];
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
