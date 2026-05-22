import type { MigrationPreview, MigrationProgress } from "./settingsModel";

interface AiSettingsDraft {
  baseUrl: string;
  model: string;
  apiKey: string;
}

interface MigrationDraft {
  baseUrl: string;
  accessToken: string;
  includeArchived: boolean;
}

interface MigrationProgressViewInput {
  previewing: boolean;
  importing: boolean;
  preview: MigrationPreview | null;
  progress: MigrationProgress | null;
}

export function buildAiSettingsPayload(draft: AiSettingsDraft) {
  return {
    baseUrl: draft.baseUrl.trim(),
    model: draft.model.trim(),
    apiKey: draft.apiKey.trim(),
  };
}

export function buildMigrationPayload(draft: MigrationDraft) {
  return {
    baseUrl: draft.baseUrl.trim(),
    accessToken: draft.accessToken.trim(),
    includeArchived: draft.includeArchived,
  };
}

export function buildMigrationProgressView({
  previewing,
  importing,
  preview,
  progress,
}: MigrationProgressViewInput) {
  const knownTotal = preview?.memoCount || 0;
  const percent = progress && knownTotal > 0
    ? Math.min(100, Math.round((progress.processed / knownTotal) * 100))
    : null;
  const title = previewing
    ? "正在预检源数据"
    : progress?.phase === "done"
      ? "迁移完成"
      : "正在迁移备忘录";
  const detail = previewing
    ? "正在读取原版 Memos 列表和元信息"
    : progress
      ? `已处理 ${progress.processed}${knownTotal ? ` / ${knownTotal}` : ""} 条，导入 ${progress.imported} 条，跳过 ${progress.skipped} 条`
      : "正在拉取并导入，完成后显示导入和跳过数量";

  return {
    visible: previewing || importing || !!progress,
    knownTotal,
    percent,
    title,
    detail,
  };
}
