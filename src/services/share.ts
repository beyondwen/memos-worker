import type { Env, Viewer, DbMemo } from "../types";
import { json, readJson, unixNow, generateUid } from "../utils";
import { getMemoByUid, canReadMemo, memoWithAttachments } from "./memo";
import { getUserById } from "../middleware";

export async function createShare(request: Request, env: Env, viewer: Viewer, memoUid: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{ expiresTs?: number }>(request);
  const uid = generateUid("s");
  const now = unixNow();

  await env.DB.prepare(`
    INSERT INTO memo_share (uid, memo_id, creator_id, created_ts, expires_ts)
    VALUES (?, ?, ?, ?, ?)
  `).bind(uid, memo.id, viewer.id, now, body.expiresTs && Number.isFinite(body.expiresTs) ? body.expiresTs : null).run();

  return json({
    share: {
      uid,
      memoUid: memo.uid,
      createdTs: now,
      expiresTs: body.expiresTs ?? null,
      url: `/api/v1/shares/${uid}`
    }
  }, 201);
}

export async function listShares(env: Env, viewer: Viewer, memoUid: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  const rows = await env.DB.prepare(`
    SELECT * FROM memo_share WHERE memo_id = ? ORDER BY created_ts DESC
  `).bind(memo.id).all<{
    id: number; uid: string; memo_id: number; creator_id: number; created_ts: number; expires_ts: number | null;
  }>();

  return json({
    shares: rows.results.map(s => ({
      id: s.id,
      uid: s.uid,
      memoUid: memo.uid,
      createdTs: s.created_ts,
      expiresTs: s.expires_ts,
      url: `/api/v1/shares/${s.uid}`
    }))
  });
}

export async function deleteShare(env: Env, viewer: Viewer, memoUid: string, shareId: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);

  const id = Number(shareId);
  if (!Number.isFinite(id)) return json({ error: "Invalid share ID" }, 400);

  const row = await env.DB.prepare(`
    SELECT id, creator_id FROM memo_share WHERE id = ? AND memo_id = ?
  `).bind(id, memo.id).first<{ id: number; creator_id: number }>();
  if (!row) return json({ error: "Share not found" }, 404);
  if (viewer.role !== "ADMIN" && row.creator_id !== viewer.id) return json({ error: "Forbidden" }, 403);

  await env.DB.prepare("DELETE FROM memo_share WHERE id = ?").bind(id).run();
  return json({ ok: true });
}

export async function getSharedMemo(env: Env, shareUid: string): Promise<Response> {
  const now = unixNow();
  const share = await env.DB.prepare(`
    SELECT * FROM memo_share WHERE uid = ? AND (expires_ts IS NULL OR expires_ts > ?)
  `).bind(shareUid, now).first<{
    id: number; uid: string; memo_id: number; creator_id: number; created_ts: number; expires_ts: number | null;
  }>();
  if (!share) return json({ error: "Share link not found or expired" }, 404);

  const memo = await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN "user" ON "user".id = memo.creator_id
    WHERE memo.id = ? AND memo.row_status = 'NORMAL'
  `).bind(share.memo_id).first<DbMemo>();
  if (!memo) return json({ error: "Memo not found" }, 404);

  return json({ memo: await memoWithAttachments(env, memo) });
}
