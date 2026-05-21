import type { Env, Viewer } from "../types";
import { json, readJson, unixNow } from "../utils";
import { recordAudit } from "./audit";

export type WebhookDeliveryStatus = "SUCCESS" | "FAILED";

interface DbWebhookDelivery {
  id: number;
  webhook_id: number;
  creator_id: number;
  created_ts: number;
  event: string;
  status: WebhookDeliveryStatus;
  status_code: number | null;
  duration_ms: number;
  error: string;
  request_body: string;
  response_body: string;
  webhook_name?: string;
  webhook_url?: string;
}

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

export async function listWebhookDeliveries(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const url = new URL(request.url);
  const webhookId = Number(url.searchParams.get("webhookId") ?? "");
  const where = ["webhook_delivery.creator_id = ?"];
  const params: unknown[] = [viewer.id];
  if (Number.isFinite(webhookId) && webhookId > 0) {
    where.push("webhook_delivery.webhook_id = ?");
    params.push(webhookId);
  }

  const rows = await env.DB.prepare(`
    SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url
    FROM webhook_delivery
    JOIN webhook ON webhook.id = webhook_delivery.webhook_id
    WHERE ${where.join(" AND ")}
    ORDER BY webhook_delivery.created_ts DESC, webhook_delivery.id DESC
    LIMIT 50
  `).bind(...params).all<DbWebhookDelivery>();

  return json({ deliveries: rows.results.map(publicWebhookDelivery) });
}

export async function retryWebhookDelivery(env: Env, viewer: Viewer, deliveryId: string): Promise<Response> {
  const id = Number(deliveryId);
  if (!Number.isFinite(id)) return json({ error: "Invalid delivery ID" }, 400);

  const delivery = await env.DB.prepare(`
    SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url
    FROM webhook_delivery
    JOIN webhook ON webhook.id = webhook_delivery.webhook_id
    WHERE webhook_delivery.id = ? AND webhook_delivery.creator_id = ?
  `).bind(id, viewer.id).first<DbWebhookDelivery>();
  if (!delivery || !delivery.webhook_url) return json({ error: "Webhook delivery not found" }, 404);

  const retried = await sendAndRecordWebhook(env, {
    webhookId: delivery.webhook_id,
    creatorId: viewer.id,
    url: delivery.webhook_url,
    event: delivery.event,
    requestBody: delivery.request_body
  });

  return json({ delivery: retried ? publicWebhookDelivery({ ...retried, webhook_name: delivery.webhook_name, webhook_url: delivery.webhook_url }) : null });
}

export async function testWebhook(env: Env, viewer: Viewer, webhookId: string): Promise<Response> {
  const id = Number(webhookId);
  if (!Number.isFinite(id)) return json({ error: "Invalid webhook ID" }, 400);

  const webhook = await env.DB.prepare("SELECT id, creator_id, name, url FROM webhook WHERE id = ? AND creator_id = ?")
    .bind(id, viewer.id)
    .first<{ id: number; creator_id: number; name: string; url: string }>();
  if (!webhook) return json({ error: "Webhook not found" }, 404);

  const delivery = await sendAndRecordWebhook(env, {
    webhookId: webhook.id,
    creatorId: webhook.creator_id,
    url: webhook.url,
    event: "webhook.test",
    requestBody: buildWebhookTestBody(unixNow())
  });

  return json({
    delivery: delivery ? publicWebhookDelivery({ ...delivery, webhook_name: webhook.name, webhook_url: webhook.url }) : null
  }, 201);
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
  await recordAudit(env, viewer, "webhook.create", String(result.meta.last_row_id), { name });

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
  await recordAudit(env, viewer, "webhook.delete", String(id));
  return json({ ok: true });
}

export async function fireWebhooks(env: Env, creatorId: number, event: string, payload: unknown): Promise<void> {
  const rows = await env.DB.prepare(`
    SELECT id, creator_id, url FROM webhook WHERE creator_id = ? AND row_status = 'NORMAL'
  `).bind(creatorId).all<{ id: number; creator_id: number; url: string }>();

  const body = JSON.stringify({ event, timestamp: unixNow(), payload });
  for (const row of rows.results) {
    await sendAndRecordWebhook(env, {
      webhookId: row.id,
      creatorId: row.creator_id,
      url: row.url,
      event,
      requestBody: body
    });
  }
}

export function deliveryStatusFromResponse(statusCode: number | null): WebhookDeliveryStatus {
  return statusCode !== null && statusCode >= 200 && statusCode < 300 ? "SUCCESS" : "FAILED";
}

export function formatWebhookError(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error.trim()) return error.trim();
  return "Unknown webhook error";
}

export function buildWebhookTestBody(timestamp: number): string {
  return JSON.stringify({
    event: "webhook.test",
    timestamp,
    payload: {
      ok: true,
      source: "memos-worker"
    }
  });
}

function publicWebhookDelivery(delivery: DbWebhookDelivery): Record<string, unknown> {
  return {
    id: delivery.id,
    webhookId: delivery.webhook_id,
    webhookName: delivery.webhook_name ?? "",
    webhookUrl: delivery.webhook_url ?? "",
    createdTs: delivery.created_ts,
    event: delivery.event,
    status: delivery.status,
    statusCode: delivery.status_code,
    durationMs: delivery.duration_ms,
    error: delivery.error,
    responseBody: delivery.response_body,
  };
}

async function sendAndRecordWebhook(env: Env, options: {
  webhookId: number;
  creatorId: number;
  url: string;
  event: string;
  requestBody: string;
}): Promise<DbWebhookDelivery | null> {
  const started = Date.now();
  let statusCode: number | null = null;
  let responseBody = "";
  let error = "";

  try {
    const response = await fetch(options.url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: options.requestBody
    });
    statusCode = response.status;
    responseBody = await response.text().catch(() => "");
    if (deliveryStatusFromResponse(statusCode) === "FAILED") {
      error = response.statusText || `HTTP ${statusCode}`;
    }
  } catch (err) {
    error = formatWebhookError(err);
  }

  const durationMs = Math.max(0, Date.now() - started);
  const status = deliveryStatusFromResponse(statusCode);
  try {
    const result = await env.DB.prepare(`
      INSERT INTO webhook_delivery (
        webhook_id, creator_id, created_ts, event, status, status_code,
        duration_ms, error, request_body, response_body
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).bind(
      options.webhookId,
      options.creatorId,
      unixNow(),
      truncateText(options.event, 200),
      status,
      statusCode,
      durationMs,
      truncateText(error, 1000),
      truncateText(options.requestBody, 12000),
      truncateText(responseBody, 4000)
    ).run();

    await pruneWebhookDeliveries(env, options.creatorId);
    const id = Number(result.meta.last_row_id);
    return await env.DB.prepare("SELECT * FROM webhook_delivery WHERE id = ?")
      .bind(id)
      .first<DbWebhookDelivery>();
  } catch (err) {
    console.warn("Webhook delivery log failed", err);
    return null;
  }
}

async function pruneWebhookDeliveries(env: Env, creatorId: number): Promise<void> {
  await env.DB.prepare(`
    DELETE FROM webhook_delivery
    WHERE creator_id = ?
      AND id NOT IN (
        SELECT id FROM webhook_delivery
        WHERE creator_id = ?
        ORDER BY created_ts DESC, id DESC
        LIMIT 200
      )
  `).bind(creatorId, creatorId).run().catch(() => undefined);
}

function truncateText(value: string, maxLength: number): string {
  return value.length > maxLength ? value.slice(0, maxLength) : value;
}
