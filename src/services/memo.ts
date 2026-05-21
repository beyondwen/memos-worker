import type { Env, Viewer, DbMemo, DbAttachment, SseEvent, Visibility } from "../types";
import {
  json, readJson, unixNow, generateUid, normalizeVisibility, normalizeState,
  safeJsonParse, encodePageToken, decodePageToken, clampNumber, HttpError
} from "../utils";
import { getUserById } from "../middleware";
import { fireWebhooks } from "./webhook";
import type { MemoWebhookEvent } from "../webhookEvents";

export function publicMemo(memo: DbMemo): Record<string, unknown> {
  return {
    name: `memos/${memo.uid}`,
    id: memo.id,
    uid: memo.uid,
    creator: {
      id: memo.creator_id,
      username: memo.creator_username,
      nickname: memo.creator_nickname
    },
    createdTs: memo.created_ts,
    updatedTs: memo.updated_ts,
    rowStatus: memo.row_status,
    content: memo.content,
    visibility: memo.visibility,
    pinned: Boolean(memo.pinned),
    payload: safeJsonParse(memo.payload, {})
  };
}

export async function memoWithAttachments(env: Env, memo: DbMemo): Promise<Record<string, unknown>> {
  const attachments = await env.DB.prepare(`
    SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id
    FROM attachment
    LEFT JOIN memo ON memo.id = attachment.memo_id
    WHERE attachment.memo_id = ?
    ORDER BY attachment.created_ts, attachment.id
  `).bind(memo.id).all<DbAttachment>();

  return {
    ...publicMemo(memo),
    attachments: attachments.results.map(publicAttachment)
  };
}

export function publicAttachment(attachment: DbAttachment): Record<string, unknown> {
  return {
    name: `attachments/${attachment.uid}`,
    uid: attachment.uid,
    filename: attachment.filename,
    type: attachment.type,
    size: attachment.size,
    memoId: attachment.memo_id,
    createdTs: attachment.created_ts,
    url: `/file/attachments/${encodeURIComponent(attachment.uid)}/${encodeURIComponent(attachment.filename)}`
  };
}

export function canReadMemo(memo: DbMemo, viewer: Viewer): boolean {
  if (viewer.role === "ADMIN") return true;
  if (memo.visibility !== "PRIVATE") return true;
  return memo.creator_id === viewer.id;
}

export function canWriteMemo(memo: DbMemo, viewer: Viewer): boolean {
  return viewer.role === "ADMIN" || memo.creator_id === viewer.id;
}

export function canReadAttachment(attachment: DbAttachment, viewer: Viewer): boolean {
  if (viewer.role === "ADMIN") return true;
  if (!attachment.memo_id) return attachment.creator_id === viewer.id;
  if (attachment.memo_visibility !== "PRIVATE") return true;
  return attachment.memo_creator_id === viewer.id;
}

export async function getMemoById(env: Env, id: number): Promise<DbMemo | null> {
  return await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN "user" ON "user".id = memo.creator_id
    WHERE memo.id = ?
  `).bind(id).first<DbMemo>();
}

export async function getMemoByUid(env: Env, uid: string): Promise<DbMemo | null> {
  return await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN "user" ON "user".id = memo.creator_id
    WHERE memo.uid = ?
  `).bind(uid).first<DbMemo>();
}

export async function attachFiles(env: Env, viewer: Viewer, memoId: number, attachmentUids: string[]): Promise<void> {
  for (const uid of attachmentUids.slice(0, 20)) {
    await env.DB.prepare(`
      UPDATE attachment SET memo_id = ?, updated_ts = ?
      WHERE uid = ? AND creator_id = ?
    `).bind(memoId, unixNow(), uid, viewer.id).run();
  }
}

export async function broadcastMemo(env: Env, type: SseEvent["type"], memo: DbMemo): Promise<void> {
  const id = env.SSE_HUB.idFromName("global");
  const event: SseEvent = {
    id: `${Date.now()}-${memo.uid}`,
    type,
    name: `memos/${memo.uid}`,
    visibility: memo.visibility,
    creatorId: memo.creator_id
  };
  await env.SSE_HUB.get(id).fetch("https://sse-hub/broadcast", {
    method: "POST",
    body: JSON.stringify(event),
    headers: { "Content-Type": "application/json" }
  }).catch((error) => {
    console.warn("SSE broadcast failed", error);
  });
}

async function emitMemoEvent(
  env: Env,
  type: SseEvent["type"],
  memo: DbMemo,
  payload: Record<string, unknown> = {}
): Promise<void> {
  await broadcastMemo(env, type, memo);
  await fireWebhooks(env, memo.creator_id, type satisfies MemoWebhookEvent, {
    memo: publicMemo(memo),
    ...payload
  });
}

export function buildMemoPayload(content: string, explicitTags?: string[]): Record<string, unknown> {
  const tags = explicitTags ?? extractTags(content);
  return {
    tags,
    property: {
      hasTaskList: /(^|\n)\s*[-*]\s+\[[ xX]\]/.test(content),
      hasLink: /https?:\/\/\S+/.test(content),
      hasCode: /```|`[^`]+`/.test(content),
      hasIncompleteTasks: /(^|\n)\s*[-*]\s+\[\s\]/.test(content)
    }
  };
}

function extractTags(content: string): string[] {
  const tags = new Set<string>();
  const regex = /(^|\s)#([\p{L}\p{N}_/-]{1,64})/gu;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(content))) tags.add(match[2]);
  return [...tags];
}

export async function listMemos(request: Request, env: Env, viewer: Viewer, filterSql?: { sql: string; params: unknown[] }): Promise<Response> {
  const url = new URL(request.url);
  const pageSize = clampNumber(Number(url.searchParams.get("page_size") ?? 50), 1, 100);
  const state = normalizeState(url.searchParams.get("state"));
  const visibility = normalizeVisibility(url.searchParams.get("visibility"), true);
  const tag = url.searchParams.get("tag")?.trim();
  const cursor = decodePageToken(url.searchParams.get("page_token"));

  const where: string[] = ["memo.row_status = ?"];
  const params: unknown[] = [state];

  if (viewer.role !== "ADMIN") {
    where.push("(memo.visibility != 'PRIVATE' OR memo.creator_id = ?)");
    params.push(viewer.id);
  }

  if (visibility) {
    where.push("memo.visibility = ?");
    params.push(visibility);
  }

  if (tag) {
    where.push("EXISTS (SELECT 1 FROM json_each(json_extract(memo.payload, '$.tags')) WHERE value = ?)");
    params.push(tag);
  }

  if (filterSql) {
    where.push(`(${filterSql.sql})`);
    params.push(...filterSql.params);
  }

  if (cursor) {
    where.push("(memo.created_ts < ? OR (memo.created_ts = ? AND memo.id < ?))");
    params.push(cursor.createdTs, cursor.createdTs, cursor.id);
  }

  const rows = await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN "user" ON "user".id = memo.creator_id
    WHERE ${where.join(" AND ")}
    ORDER BY memo.pinned DESC, memo.created_ts DESC, memo.id DESC
    LIMIT ?
  `).bind(...params, pageSize + 1).all<DbMemo>();

  const memos = rows.results.slice(0, pageSize);
  const last = memos[memos.length - 1];
  return json({
    memos: await Promise.all(memos.map((memo) => memoWithAttachments(env, memo))),
    nextPageToken: rows.results.length > pageSize && last ? encodePageToken(last.created_ts, last.id) : ""
  });
}

export async function createMemo(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const body = await readJson<{ content?: string; visibility?: string; pinned?: boolean; attachmentUids?: string[] }>(request);
  const content = String(body.content ?? "").trim();
  if (!content) return json({ error: "Content is required" }, 400);

  const visibility = normalizeVisibility(body.visibility, false) ?? "PRIVATE";
  const uid = generateUid("m");
  const now = unixNow();
  const payload = buildMemoPayload(content);

  const result = await env.DB.prepare(`
    INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, pinned, payload)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
  `).bind(uid, viewer.id, now, now, content, visibility, body.pinned ? 1 : 0, JSON.stringify(payload)).run();

  const memo = await getMemoById(env, Number(result.meta.last_row_id));
  if (body.attachmentUids?.length) {
    await attachFiles(env, viewer, Number(result.meta.last_row_id), body.attachmentUids);
  }
  if (memo) await emitMemoEvent(env, "memo.created", memo);
  return json({ memo: memo ? await memoWithAttachments(env, memo) : null }, 201);
}

export async function getMemo(env: Env, viewer: Viewer, uid: string): Promise<Response> {
  const memo = await getMemoByUid(env, uid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);
  return json({ memo: await memoWithAttachments(env, memo) });
}

export async function updateMemo(request: Request, env: Env, viewer: Viewer, uid: string): Promise<Response> {
  const memo = await getMemoByUid(env, uid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canWriteMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{
    content?: string;
    visibility?: string;
    pinned?: boolean;
    rowStatus?: string;
    attachmentUids?: string[];
  }>(request);

  const nextContent = body.content === undefined ? memo.content : String(body.content).trim();
  if (!nextContent) return json({ error: "Content is required" }, 400);

  const nextVisibility = body.visibility === undefined
    ? memo.visibility
    : normalizeVisibility(body.visibility, false) ?? memo.visibility;
  const nextStatus = body.rowStatus === undefined ? memo.row_status : normalizeState(body.rowStatus);
  const nextPinned = body.pinned === undefined ? memo.pinned : body.pinned ? 1 : 0;

  await env.DB.prepare(`
    UPDATE memo
    SET updated_ts = ?, content = ?, visibility = ?, pinned = ?, row_status = ?, payload = ?
    WHERE id = ?
  `).bind(unixNow(), nextContent, nextVisibility, nextPinned, nextStatus, JSON.stringify(buildMemoPayload(nextContent)), memo.id).run();

  if (body.attachmentUids) await attachFiles(env, viewer, memo.id, body.attachmentUids);

  const updated = await getMemoByUid(env, uid);
  if (updated) {
    const eventType: SseEvent["type"] = memo.row_status !== nextStatus
      ? nextStatus === "ARCHIVED" ? "memo.archived" : "memo.restored"
      : "memo.updated";
    await emitMemoEvent(env, eventType, updated);
  }
  return json({ memo: updated ? await memoWithAttachments(env, updated) : null });
}

export async function deleteMemo(env: Env, viewer: Viewer, uid: string): Promise<Response> {
  const memo = await getMemoByUid(env, uid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canWriteMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  await archiveMemoRecord(env, memo);
  await emitMemoEvent(env, "memo.archived", { ...memo, row_status: "ARCHIVED", updated_ts: unixNow() });
  return json({ ok: true });
}

export async function purgeMemo(env: Env, viewer: Viewer, uid: string): Promise<Response> {
  const memo = await getMemoByUid(env, uid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canWriteMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  await purgeMemoRecord(env, memo);
  await emitMemoEvent(env, "memo.deleted", memo, { hardDelete: true });
  return json({ ok: true });
}

export async function bulkUpdateMemos(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const body = await readJson<{
    action?: string;
    memoUids?: string[];
    visibility?: string;
  }>(request);
  const action = String(body.action ?? "").toUpperCase();
  const memoUids = [...new Set((body.memoUids ?? []).map((uid) => String(uid).trim()).filter(Boolean))].slice(0, 100);
  if (!["ARCHIVE", "RESTORE", "DELETE", "VISIBILITY"].includes(action)) {
    return json({ error: "Invalid bulk action" }, 400);
  }
  if (memoUids.length === 0) return json({ error: "memoUids is required" }, 400);

  const visibility = action === "VISIBILITY" ? normalizeVisibility(body.visibility, false) : null;
  if (action === "VISIBILITY" && !visibility) return json({ error: "Invalid visibility" }, 400);

  const result = { updated: 0, deleted: 0, skipped: 0 };
  const touched: DbMemo[] = [];

  for (const uid of memoUids) {
    const memo = await getMemoByUid(env, uid);
    if (!memo || !canWriteMemo(memo, viewer)) {
      result.skipped += 1;
      continue;
    }

    if (action === "DELETE") {
      await purgeMemoRecord(env, memo);
      result.deleted += 1;
      touched.push(memo);
      await broadcastMemo(env, "memo.deleted", memo);
      continue;
    }

    if (action === "ARCHIVE") {
      await archiveMemoRecord(env, memo);
    } else if (action === "RESTORE") {
      await restoreMemoRecord(env, memo);
    } else if (action === "VISIBILITY" && visibility) {
      await setMemoVisibility(env, memo, visibility);
    }

    const updated = await getMemoByUid(env, uid);
    if (updated) {
      result.updated += 1;
      touched.push(updated);
      const eventType: SseEvent["type"] = action === "ARCHIVE"
        ? "memo.archived"
        : action === "RESTORE"
          ? "memo.restored"
          : "memo.updated";
      await broadcastMemo(env, eventType, updated);
    }
  }

  if (touched.length > 0) {
    await fireWebhooks(env, viewer.id, "memo.bulk.updated", {
      action,
      memoUids: touched.map((memo) => memo.uid),
      result
    });
  }

  return json(result);
}

async function archiveMemoRecord(env: Env, memo: DbMemo): Promise<void> {
  await env.DB.prepare("UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id = ?")
    .bind(unixNow(), memo.id)
    .run();
}

async function restoreMemoRecord(env: Env, memo: DbMemo): Promise<void> {
  await env.DB.prepare("UPDATE memo SET row_status = 'NORMAL', updated_ts = ? WHERE id = ?")
    .bind(unixNow(), memo.id)
    .run();
}

async function setMemoVisibility(env: Env, memo: DbMemo, visibility: Visibility): Promise<void> {
  await env.DB.prepare("UPDATE memo SET visibility = ?, updated_ts = ? WHERE id = ?")
    .bind(visibility, unixNow(), memo.id)
    .run();
}

async function purgeMemoRecord(env: Env, memo: DbMemo): Promise<void> {
  const now = unixNow();
  await env.DB.prepare("UPDATE attachment SET memo_id = NULL, updated_ts = ? WHERE memo_id = ?")
    .bind(now, memo.id)
    .run();
  await env.DB.prepare("DELETE FROM reaction WHERE content_type = 'MEMO' AND content_id = ?")
    .bind(memo.id)
    .run();
  await env.DB.prepare("DELETE FROM memo_share WHERE memo_id = ?")
    .bind(memo.id)
    .run();
  await env.DB.prepare("DELETE FROM memo_relation WHERE memo_id = ? OR related_memo_id = ?")
    .bind(memo.id, memo.id)
    .run();
  await env.DB.prepare("DELETE FROM memo WHERE id = ?")
    .bind(memo.id)
    .run();
}

export async function exportData(env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
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

  return json({
    exportedAt: new Date().toISOString(),
    users: users.results,
    memos: memos.results,
    attachments: attachments.results
  });
}

export async function importData(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const body = await readJson<{
    memos?: Array<Partial<DbMemo> & { createdAt?: string; tags?: string[] }>;
  }>(request);

  let imported = 0;
  const now = unixNow();

  for (const item of body.memos ?? []) {
    const content = String(item.content ?? "").trim();
    if (!content) continue;
    const uid = item.uid || generateUid("m");
    const createdTs = Number(item.created_ts ?? (item.createdAt ? Math.floor(Date.parse(item.createdAt) / 1000) : now));
    const vis = normalizeVisibility(item.visibility, false) ?? "PRIVATE";
    const payload = item.payload ? safeJsonParse(item.payload, buildMemoPayload(content)) : buildMemoPayload(content, item.tags);

    await env.DB.prepare(`
      INSERT OR IGNORE INTO memo (uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).bind(
      uid,
      viewer.id,
      Number.isFinite(createdTs) ? createdTs : now,
      Number(item.updated_ts ?? createdTs ?? now),
      normalizeState(item.row_status),
      content,
      vis,
      item.pinned ? 1 : 0,
      JSON.stringify(payload)
    ).run();
    imported += 1;
  }

  return json({ imported });
}
