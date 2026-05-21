import type { Env, Viewer } from "../types";
import { json, readJson, unixNow } from "../utils";

export async function listWebhooks(env: Env, viewer: Viewer): Promise<Response> {
  const rows = await env.DB.prepare(`
    SELECT * FROM webhook WHERE creator_id = ? ORDER BY created_ts DESC
  `).bind(viewer.id).all<{
    id: number; created_ts: number; updated_ts: number; row_status: string; creator_id: number; name: string; url: string;
  }>();

  return json({
    webhooks: rows.results.map(w => ({
      id: w.id,
      name: w.name,
      url: w.url,
      rowStatus: w.row_status,
      createdTs: w.created_ts,
      updatedTs: w.updated_ts
    }))
  });
}

export async function createWebhook(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const body = await readJson<{ name?: string; url?: string }>(request);
  const name = String(body.name ?? "").trim();
  const url = String(body.url ?? "").trim();
  if (!name) return json({ error: "Name is required" }, 400);
  if (!url) return json({ error: "URL is required" }, 400);

  const now = unixNow();
  const result = await env.DB.prepare(`
    INSERT INTO webhook (created_ts, updated_ts, creator_id, name, url) VALUES (?, ?, ?, ?, ?)
  `).bind(now, now, viewer.id, name, url).run();

  return json({
    webhook: { id: Number(result.meta.last_row_id), name, url, rowStatus: "NORMAL", createdTs: now, updatedTs: now }
  }, 201);
}

export async function updateWebhook(request: Request, env: Env, viewer: Viewer, webhookId: string): Promise<Response> {
  const id = Number(webhookId);
  if (!Number.isFinite(id)) return json({ error: "Invalid webhook ID" }, 400);

  const existing = await env.DB.prepare("SELECT * FROM webhook WHERE id = ? AND creator_id = ?")
    .bind(id, viewer.id).first<{ id: number; name: string; url: string; row_status: string }>();
  if (!existing) return json({ error: "Webhook not found" }, 404);

  const body = await readJson<{ name?: string; url?: string; rowStatus?: string }>(request);
  const now = unixNow();
  const nextName = body.name !== undefined ? String(body.name).trim() : existing.name;
  const nextUrl = body.url !== undefined ? String(body.url).trim() : existing.url;
  const nextStatus = body.rowStatus === "ARCHIVED" || body.rowStatus === "NORMAL"
    ? body.rowStatus
    : existing.row_status;

  await env.DB.prepare(`
    UPDATE webhook SET name = ?, url = ?, row_status = ?, updated_ts = ? WHERE id = ?
  `).bind(nextName, nextUrl, nextStatus, now, id).run();

  return json({ webhook: { id, name: nextName, url: nextUrl, rowStatus: nextStatus, updatedTs: now } });
}

export async function deleteWebhook(env: Env, viewer: Viewer, webhookId: string): Promise<Response> {
  const id = Number(webhookId);
  if (!Number.isFinite(id)) return json({ error: "Invalid webhook ID" }, 400);

  const existing = await env.DB.prepare("SELECT id FROM webhook WHERE id = ? AND creator_id = ?")
    .bind(id, viewer.id).first();
  if (!existing) return json({ error: "Webhook not found" }, 404);

  await env.DB.prepare("DELETE FROM webhook WHERE id = ?").bind(id).run();
  return json({ ok: true });
}

export async function fireWebhooks(env: Env, creatorId: number, event: string, payload: unknown): Promise<void> {
  const rows = await env.DB.prepare(`
    SELECT url FROM webhook WHERE creator_id = ? AND row_status = 'NORMAL'
  `).bind(creatorId).all<{ url: string }>();

  const body = JSON.stringify({ event, timestamp: unixNow(), payload });
  for (const row of rows.results) {
    try {
      await fetch(row.url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body
      }).catch(() => undefined);
    } catch {
      // ignore webhook failures
    }
  }
}
