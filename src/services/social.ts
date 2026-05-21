import type { Env, Viewer, DbMemo } from "../types";
import { json, readJson, unixNow, generateUid } from "../utils";
import { getMemoByUid, getMemoById, publicMemo, memoWithAttachments, canReadMemo, canWriteMemo, broadcastMemo } from "./memo";
import { getUserById } from "../middleware";
import { fireWebhooks } from "./webhook";

export async function createComment(request: Request, env: Env, viewer: Viewer, parentUid: string): Promise<Response> {
  const parent = await getMemoByUid(env, parentUid);
  if (!parent) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(parent, viewer)) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{ content?: string; visibility?: string }>(request);
  const content = String(body.content ?? "").trim();
  if (!content) return json({ error: "Content is required" }, 400);

  const uid = generateUid("m");
  const now = unixNow();
  const { buildMemoPayload } = await import("./memo");
  const payload = buildMemoPayload(content);

  const result = await env.DB.prepare(`
    INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, payload)
    VALUES (?, ?, ?, ?, ?, 'PRIVATE', ?)
  `).bind(uid, viewer.id, now, now, content, JSON.stringify(payload)).run();

  const commentId = Number(result.meta.last_row_id);

  await env.DB.prepare(`
    INSERT INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'COMMENT')
  `).bind(parent.id, commentId).run();

  const comment = await getMemoById(env, commentId);

  await env.DB.prepare(`
    INSERT INTO inbox (sender_id, receiver_id, status, message)
    VALUES (?, ?, 'UNREAD', ?)
  `).bind(viewer.id, parent.creator_id, JSON.stringify({
    type: "memo.comment.created",
    memoUid: parentUid,
    commentUid: uid
  })).run();

  if (comment) {
    await broadcastMemo(env, "memo.created", comment);
    await broadcastMemo(env, "memo.comment.created", parent);
    await fireWebhooks(env, comment.creator_id, "memo.created", { memo: publicMemo(comment) });
    await fireWebhooks(env, parent.creator_id, "memo.comment.created", {
      memo: publicMemo(parent),
      comment: publicMemo(comment)
    });
  }
  return json({ memo: comment ? await memoWithAttachments(env, comment) : null }, 201);
}

export async function listComments(env: Env, viewer: Viewer, parentUid: string): Promise<Response> {
  const parent = await getMemoByUid(env, parentUid);
  if (!parent) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(parent, viewer)) return json({ error: "Forbidden" }, 403);

  const rows = await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN memo_relation ON memo_relation.related_memo_id = memo.id
    JOIN "user" ON "user".id = memo.creator_id
    WHERE memo_relation.memo_id = ? AND memo_relation.type = 'COMMENT' AND memo.row_status = 'NORMAL'
    ORDER BY memo.created_ts ASC
  `).bind(parent.id).all<DbMemo>();

  return json({
    memos: await Promise.all(rows.results.map(m => memoWithAttachments(env, m)))
  });
}

export async function upsertReaction(request: Request, env: Env, viewer: Viewer, memoUid: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{ reactionType?: string }>(request);
  const reactionType = String(body.reactionType ?? "").trim();
  if (!reactionType) return json({ error: "reactionType is required" }, 400);

  const now = unixNow();
  await env.DB.prepare(`
    INSERT INTO reaction (created_ts, creator_id, content_type, content_id, reaction_type)
    VALUES (?, ?, 'MEMO', ?, ?)
    ON CONFLICT (creator_id, content_type, content_id, reaction_type) DO NOTHING
  `).bind(now, viewer.id, memo.id, reactionType).run();

  await broadcastMemo(env, "reaction.upserted", memo);
  await fireWebhooks(env, memo.creator_id, "reaction.upserted", {
    memo: publicMemo(memo),
    reactionType,
    actorId: viewer.id
  });
  return await listReactionsForMemo(env, memo.id);
}

export async function deleteReaction(env: Env, viewer: Viewer, memoUid: string, reactionId: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);

  const id = Number(reactionId);
  if (!Number.isFinite(id)) return json({ error: "Invalid reaction ID" }, 400);

  const row = await env.DB.prepare(`
    SELECT id, creator_id FROM reaction WHERE id = ? AND content_type = 'MEMO' AND content_id = ?
  `).bind(id, memo.id).first<{ id: number; creator_id: number }>();
  if (!row) return json({ error: "Reaction not found" }, 404);
  if (viewer.role !== "ADMIN" && row.creator_id !== viewer.id) return json({ error: "Forbidden" }, 403);

  await env.DB.prepare("DELETE FROM reaction WHERE id = ?").bind(id).run();
  await broadcastMemo(env, "reaction.deleted", memo);
  await fireWebhooks(env, memo.creator_id, "reaction.deleted", {
    memo: publicMemo(memo),
    reactionId: id,
    actorId: viewer.id
  });
  return await listReactionsForMemo(env, memo.id);
}

export async function listReactions(env: Env, viewer: Viewer, memoUid: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);
  return await listReactionsForMemo(env, memo.id);
}

async function listReactionsForMemo(env: Env, memoId: number): Promise<Response> {
  const rows = await env.DB.prepare(`
    SELECT reaction.id, reaction.created_ts, reaction.reaction_type, reaction.creator_id,
           "user".username AS creator_username
    FROM reaction
    JOIN "user" ON "user".id = reaction.creator_id
    WHERE reaction.content_type = 'MEMO' AND reaction.content_id = ?
    ORDER BY reaction.created_ts ASC
  `).bind(memoId).all<{
    id: number; created_ts: number; reaction_type: string; creator_id: number; creator_username: string;
  }>();

  return json({
    reactions: rows.results.map(r => ({
      id: r.id,
      reactionType: r.reaction_type,
      creator: { id: r.creator_id, username: r.creator_username },
      createdTs: r.created_ts
    }))
  });
}

export async function getRelations(env: Env, viewer: Viewer, memoUid: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  const refs = await env.DB.prepare(`
    SELECT memo.uid, memo.content, memo_relation.type
    FROM memo_relation
    JOIN memo ON memo.id = memo_relation.related_memo_id
    WHERE memo_relation.memo_id = ? AND memo_relation.type = 'REFERENCE'
  `).bind(memo.id).all<{ uid: string; content: string; type: string }>();

  const backRefs = await env.DB.prepare(`
    SELECT memo.uid, memo.content, memo_relation.type
    FROM memo_relation
    JOIN memo ON memo.id = memo_relation.memo_id
    WHERE memo_relation.related_memo_id = ? AND memo_relation.type = 'REFERENCE'
  `).bind(memo.id).all<{ uid: string; content: string; type: string }>();

  return json({
    relations: [
      ...refs.results.map(r => ({ memo: `memos/${r.uid}`, type: r.type, direction: "outgoing" as const, content: r.content })),
      ...backRefs.results.map(r => ({ memo: `memos/${r.uid}`, type: r.type, direction: "incoming" as const, content: r.content }))
    ]
  });
}

export async function setRelations(request: Request, env: Env, viewer: Viewer, memoUid: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canWriteMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{ relations?: Array<{ memo: string; type?: string }> }>(request);

  await env.DB.prepare("DELETE FROM memo_relation WHERE memo_id = ? AND type = 'REFERENCE'").bind(memo.id).run();

  for (const rel of body.relations ?? []) {
    const relatedUid = String(rel.memo ?? "").replace(/^memos\//, "");
    if (!relatedUid || relatedUid === memoUid) continue;
    const related = await getMemoByUid(env, relatedUid);
    if (!related) continue;

    await env.DB.prepare(`
      INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'REFERENCE')
    `).bind(memo.id, related.id).run();
  }

  return await getRelations(env, viewer, memoUid);
}
