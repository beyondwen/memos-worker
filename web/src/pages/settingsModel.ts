export interface Pat {
  id: number;
  name: string;
  prefix: string;
  createdTs: number;
  expiresTs: number | null;
  rowStatus: string;
}

export interface NewPat {
  id: number;
  name: string;
  token: string;
  prefix: string;
  createdTs: number;
  expiresTs: number | null;
}

export interface Webhook {
  id: number;
  name: string;
  url: string;
  rowStatus: "NORMAL" | "ARCHIVED";
  createdTs: number;
  updatedTs: number;
}

export interface WebhookDelivery {
  id: number;
  webhookId: number;
  webhookName: string;
  webhookUrl: string;
  createdTs: number;
  event: string;
  status: "SUCCESS" | "FAILED";
  statusCode: number | null;
  durationMs: number;
  error: string;
  responseBody: string;
}

export interface Attachment {
  uid: string;
  filename: string;
  type: string;
  size: number;
  createdTs: number;
  url: string;
}

export interface UserStats {
  memoCount: number;
  attachmentCount: number;
}

export interface BackupItem {
  key: string;
  size: number;
  uploaded: string;
}

export interface BackupPreview {
  userCount: number;
  memoCount: number;
  attachmentCount: number;
  relationCount: number;
}

export interface MigrationPreview {
  memoCount: number;
  attachmentCount: number;
  relationCount: number;
  archivedCount: number;
  truncated: boolean;
}

export interface MigrationResult extends MigrationPreview {
  imported: number;
  skipped: number;
}

export interface MigrationProgress extends MigrationResult {
  phase: "fetching" | "importing" | "done";
  processed: number;
  state?: string;
}

export interface AiSettings {
  baseUrl: string;
  model: string;
  configured: boolean;
}

export interface TagItem {
  name: string;
  count: number;
}

export interface AuditLog {
  id: number;
  createdTs: number;
  actorUsername: string | null;
  action?: string;
  actionLabel: string;
  target: string;
  detail?: Record<string, unknown>;
}

export type SettingsTab = "account" | "integrations" | "data" | "maintenance" | "audit";

export const SETTINGS_TABS: Array<{
  id: SettingsTab;
  label: string;
  description: string;
  adminOnly?: boolean;
}> = [
  { id: "account", label: "账号", description: "资料、密码和访问令牌" },
  { id: "integrations", label: "集成", description: "Webhook 和投递记录" },
  { id: "data", label: "数据", description: "导入导出、备份和标签", adminOnly: true },
  { id: "maintenance", label: "维护", description: "附件清理" },
  { id: "audit", label: "审计", description: "操作记录", adminOnly: true },
];

export function parseMigrationStreamEvent(raw: string): { name: string; data: unknown } {
  let name = "message";
  const dataLines: string[] = [];
  for (const line of raw.split("\n")) {
    if (line.startsWith("event:")) name = line.slice("event:".length).trim();
    if (line.startsWith("data:")) dataLines.push(line.slice("data:".length).trim());
  }
  return {
    name,
    data: JSON.parse(dataLines.join("\n") || "{}"),
  };
}

export function auditLogDetail(log: AuditLog): string {
  const detail = log.detail ?? {};
  if (log.action?.startsWith("migration.usememos")) {
    const imported = Number(detail.imported ?? 0);
    const skipped = Number(detail.skipped ?? 0);
    const total = Number(detail.memoCount ?? 0);
    const error = typeof detail.error === "string" ? detail.error : "";
    if (error) return error;
    if (total || imported || skipped) return `导入 ${imported}，跳过 ${skipped}，总计 ${total}`;
    const baseUrl = typeof detail.baseUrl === "string" ? detail.baseUrl : "";
    return baseUrl ? `来源 ${baseUrl}` : log.target;
  }
  if (log.action?.startsWith("backup.")) {
    const size = Number(detail.size ?? 0);
    return size ? `${log.target} · ${Math.round(size / 1024)} KB` : log.target;
  }
  return log.target;
}
