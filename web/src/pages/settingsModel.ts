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

export interface UserSession {
  id: string;
  createdTs: number;
  updatedTs: number;
  lastUsedTs?: number | null;
  expiresTs: number;
  userAgent: string;
  rowStatus: string;
  current: boolean;
}

export interface BackupItem {
  key: string;
  size: number;
  uploaded: string;
  encrypted?: boolean;
  keyId?: string | null;
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

export interface OriginalBackupResult {
  memoCount: number;
  pushed: number;
  skipped: number;
  archivedCount: number;
  truncated: boolean;
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

export interface SystemHealth {
  status: "healthy" | "degraded";
  checkedTs: number;
  memoIndex: {
    memoCount: number;
    searchCount: number;
    missingSearchCount: number;
    orphanSearchCount: number;
    tagCount: number;
    orphanTagCount: number;
    healthy: boolean;
  };
  backup: {
    r2Available: boolean;
    count: number;
    latest?: {
      key: string;
      size: number;
      uploaded: string;
    } | null;
    encryption: {
      configured: boolean;
      currentKeyId?: string | null;
      knownKeyIds: string[];
    };
  };
}

export interface RelationRebuildProgress {
  taskId: string;
  status: "RUNNING" | "DONE" | "FAILED" | "CANCELED";
  mode: "supplement" | "replace";
  total: number;
  processed: number;
  batchProcessed: number;
  created: number;
  updated: number;
  skipped: number;
  nextCursor?: number | null;
  done: boolean;
  source: "ai" | "local";
  warnings: string[];
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

export type SettingsTab = "account" | "data" | "maintenance" | "audit";

export const SETTINGS_TABS: Array<{
  id: SettingsTab;
  label: string;
  description: string;
  adminOnly?: boolean;
}> = [
  { id: "account", label: "账号", description: "资料和密码" },
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
    const pushed = Number(detail.pushed ?? 0);
    const skipped = Number(detail.skipped ?? 0);
    const total = Number(detail.memoCount ?? 0);
    const error = typeof detail.error === "string" ? detail.error : "";
    if (error) return error;
    if (log.action === "migration.usememos.export") return `备份 ${pushed}，跳过 ${skipped}，总计 ${total}`;
    if (total || imported || skipped) return `导入 ${imported}，跳过 ${skipped}，总计 ${total}`;
    const baseUrl = typeof detail.baseUrl === "string" ? detail.baseUrl : "";
    return baseUrl ? `来源 ${baseUrl}` : log.target;
  }
  if (log.action?.startsWith("backup.")) {
    const size = Number(detail.size ?? 0);
    return size ? `${log.target} · ${Math.round(size / 1024)} KB` : log.target;
  }
  if (log.action === "relations.rebuild") {
    const created = Number(detail.created ?? 0);
    const total = Number(detail.total ?? 0);
    return `处理 ${total} 篇，写入 ${created} 条关联`;
  }
  return log.target;
}
