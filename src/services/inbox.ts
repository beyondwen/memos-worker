import type { Env, Viewer } from "../types";
import { json, readJson, unixNow } from "../utils";

interface InboxMessage {
  type: string;
  memoUid?: string;
  commentUid?: string;
  [key: string]: unknown;
}

interface DbInboxRow {
  id: number;
  created_ts: number;
  sender_id: number | null;
  receiver_id: number;
  status: "UNREAD" | "READ";
  message: string;
  sender_username?: string;
  sender_nickname?: string;
}

export async function listInbox(env: Env, viewer: Viewer): Promise<Response> {
  const rows = await env.DB.prepare(`
    SELECT inbox.*, "user".username AS sender_username, "user".nickname AS sender_nickname
    FROM inbox
    LEFT JOIN "user" ON "user".id = inbox.sender_id
    WHERE inbox.receiver_id = ?
    ORDER BY inbox.created_ts DESC
    LIMIT 100
  `).bind(viewer.id).all<DbInboxRow>();

  const unreadCount = await env.DB.prepare(`
    SELECT COUNT(*) AS count FROM inbox WHERE receiver_id = ? AND status = 'UNREAD'
  `).bind(viewer.id).first<{ count: number }>();

  return json({
    inbox: rows.results.map(row => ({
      id: row.id,
      createdTs: row.created_ts,
      sender: row.sender_id ? { id: row.sender_id, username: row.sender_username, nickname: row.sender_nickname } : null,
      status: row.status,
      message: safeParseMessage(row.message)
    })),
    unreadCount: unreadCount?.count ?? 0
  });
}

export async function updateInboxStatus(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const body = await readJson<{ ids?: number[]; status?: string }>(request);
  const status = body.status === "READ" ? "READ" : "UNREAD";

  if (body.ids?.length) {
    const placeholders = body.ids.map(() => "?").join(",");
    await env.DB.prepare(`
      UPDATE inbox SET status = ? WHERE id IN (${placeholders}) AND receiver_id = ?
    `).bind(status, ...body.ids, viewer.id).run();
  } else {
    await env.DB.prepare(`
      UPDATE inbox SET status = ? WHERE receiver_id = ?
    `).bind(status, viewer.id).run();
  }

  return json({ ok: true });
}

export async function deleteInboxItem(env: Env, viewer: Viewer, itemId: string): Promise<Response> {
  const id = Number(itemId);
  if (!Number.isFinite(id)) return json({ error: "Invalid inbox ID" }, 400);

  await env.DB.prepare("DELETE FROM inbox WHERE id = ? AND receiver_id = ?").bind(id, viewer.id).run();
  return json({ ok: true });
}

function safeParseMessage(value: string): InboxMessage {
  try {
    return JSON.parse(value) as InboxMessage;
  } catch {
    return { type: "unknown" };
  }
}
