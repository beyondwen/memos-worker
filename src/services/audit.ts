import type { Env, Viewer } from "../types";
import { json, unixNow } from "../utils";

export function auditActionLabel(action: string): string {
  return {
    "memo.delete": "删除备忘录",
    "memo.purge": "彻底删除备忘录",
    "attachment.delete": "删除附件",
    "backup.create": "创建备份",
    "backup.restore": "恢复备份",
    "migration.usememos.import": "迁移原版 Memos",
    "webhook.create": "创建 Webhook",
    "webhook.delete": "删除 Webhook",
    "tag.rename": "重命名标签",
  }[action] ?? action;
}

export async function recordAudit(env: Env, viewer: Viewer | null, action: string, target: string, detail: unknown = {}): Promise<void> {
  await env.DB.prepare(`
    INSERT INTO audit_log (created_ts, actor_id, action, target, detail)
    VALUES (?, ?, ?, ?, ?)
  `).bind(unixNow(), viewer?.id ?? null, action, target, JSON.stringify(detail)).run().catch(() => undefined);
}

export async function listAuditLogs(env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const rows = await env.DB.prepare(`
    SELECT audit_log.*, "user".username AS actor_username
    FROM audit_log
    LEFT JOIN "user" ON "user".id = audit_log.actor_id
    ORDER BY audit_log.created_ts DESC, audit_log.id DESC
    LIMIT 100
  `).all<{
    id: number; created_ts: number; actor_id: number | null; actor_username: string | null;
    action: string; target: string; detail: string;
  }>();
  return json({
    logs: rows.results.map((row) => ({
      id: row.id,
      createdTs: row.created_ts,
      actorId: row.actor_id,
      actorUsername: row.actor_username,
      action: row.action,
      actionLabel: auditActionLabel(row.action),
      target: row.target,
      detail: JSON.parse(row.detail || "{}")
    }))
  });
}
