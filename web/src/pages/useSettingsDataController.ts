import { useCallback, useEffect, useMemo, useState } from "preact/hooks";
import { api } from "../api";
import type { CurrentUser } from "../App";
import type {
  AiSettings,
  BackupItem,
  BackupPreview,
  MigrationPreview,
  MigrationProgress,
  MigrationResult,
  OriginalBackupResult,
  RelationRebuildProgress,
  SystemHealth,
  TagItem,
} from "./settingsModel";
import { runMigrationStream } from "./settingsMigration";
import { buildAiSettingsPayload, buildMigrationPayload, buildMigrationProgressView } from "./settingsPageHelpers";
import { reportSettingsLoadError } from "./settingsErrors";

type Notify = (message: string, kind?: "success" | "error" | "info") => void;
type Confirm = (options: {
  title: string;
  message?: string;
  confirmText?: string;
  danger?: boolean;
}) => Promise<boolean>;

interface UseSettingsDataControllerOptions {
  currentUser: CurrentUser | null;
  notify: Notify;
  confirm: Confirm;
  refreshAuditLogs: () => Promise<void>;
  refreshOverview: () => Promise<void>;
}

export function useSettingsDataController({
  currentUser,
  notify,
  confirm,
  refreshAuditLogs,
  refreshOverview,
}: UseSettingsDataControllerOptions) {
  const [backupCreating, setBackupCreating] = useState(false);
  const [backups, setBackups] = useState<BackupItem[]>([]);
  const [backupPreview, setBackupPreview] = useState<BackupPreview | null>(null);
  const [restoringBackupKey, setRestoringBackupKey] = useState("");
  const [migrationBaseUrl, setMigrationBaseUrl] = useState("");
  const [migrationToken, setMigrationToken] = useState("");
  const [migrationIncludeArchived, setMigrationIncludeArchived] = useState(false);
  const [migrationPreview, setMigrationPreview] = useState<MigrationPreview | null>(null);
  const [migrationResult, setMigrationResult] = useState<MigrationResult | null>(null);
  const [migrationProgress, setMigrationProgress] = useState<MigrationProgress | null>(null);
  const [originalBackupResult, setOriginalBackupResult] = useState<OriginalBackupResult | null>(null);
  const [migrationPreviewing, setMigrationPreviewing] = useState(false);
  const [migrationImporting, setMigrationImporting] = useState(false);
  const [originalBackuping, setOriginalBackuping] = useState(false);
  const [aiBaseUrl, setAiBaseUrl] = useState("https://api.openai.com/v1");
  const [aiModel, setAiModel] = useState("gpt-4o-mini");
  const [aiApiKey, setAiApiKey] = useState("");
  const [aiConfigured, setAiConfigured] = useState(false);
  const [aiSaving, setAiSaving] = useState(false);
  const [aiTesting, setAiTesting] = useState(false);
  const [tags, setTags] = useState<TagItem[]>([]);
  const [tagFrom, setTagFrom] = useState("");
  const [tagTo, setTagTo] = useState("");
  const [tagSaving, setTagSaving] = useState(false);
  const [systemHealth, setSystemHealth] = useState<SystemHealth | null>(null);
  const [healthLoading, setHealthLoading] = useState(false);
  const [rebuildingIndex, setRebuildingIndex] = useState(false);
  const [rebuildingRelations, setRebuildingRelations] = useState(false);
  const [relationRebuildProgress, setRelationRebuildProgress] = useState<RelationRebuildProgress | null>(null);

  const fetchBackups = useCallback(async () => {
    if (!currentUser || currentUser.role !== "ADMIN") return;
    try {
      const data = await api<{ backups: BackupItem[] }>("/api/v1/backups");
      setBackups(data.backups);
    } catch (err) {
      reportSettingsLoadError("备份列表", err, notify);
    }
  }, [currentUser, notify]);

  const fetchAiSettings = useCallback(async () => {
    if (!currentUser || currentUser.role !== "ADMIN") return;
    try {
      const data = await api<{ settings: AiSettings }>("/api/v1/ai/settings");
      setAiBaseUrl(data.settings.baseUrl);
      setAiModel(data.settings.model);
      setAiConfigured(data.settings.configured);
      setAiApiKey("");
    } catch (err) {
      reportSettingsLoadError("AI 设置", err, notify);
    }
  }, [currentUser, notify]);

  const fetchTags = useCallback(async () => {
    if (!currentUser) return;
    try {
      const data = await api<{ tags: TagItem[] }>("/api/v1/tags");
      setTags(data.tags);
    } catch (err) {
      reportSettingsLoadError("标签列表", err);
    }
  }, [currentUser]);

  const fetchSystemHealth = useCallback(async () => {
    if (!currentUser || currentUser.role !== "ADMIN") return;
    setHealthLoading(true);
    try {
      const data = await api<SystemHealth>("/api/v1/system/health");
      setSystemHealth(data);
    } catch (err) {
      reportSettingsLoadError("系统健康", err, notify);
    } finally {
      setHealthLoading(false);
    }
  }, [currentUser, notify]);

  useEffect(() => {
    fetchBackups();
    fetchAiSettings();
    fetchTags();
    fetchSystemHealth();
  }, [fetchAiSettings, fetchBackups, fetchSystemHealth, fetchTags]);

  const handleExport = async () => {
    try {
      const data = await api<unknown>("/api/v1/export/memos");
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `memos-export-${new Date().toISOString().slice(0, 10)}.json`;
      link.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      notify(`导出失败：${(err as Error).message}`, "error");
    }
  };

  const handleImport = async (e: Event) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      const payload = JSON.parse(await file.text());
      const result = await api<{ imported: number }>("/api/v1/import/memos", {
        method: "POST",
        body: JSON.stringify(payload),
      });
      notify(`已导入 ${result.imported} 条备忘录`, "success");
    } catch (err) {
      notify(`导入失败：${(err as Error).message}`, "error");
    } finally {
      input.value = "";
    }
  };

  const handleSaveAiSettings = async () => {
    setAiSaving(true);
    try {
      const data = await api<{ settings: AiSettings }>("/api/v1/ai/settings", {
        method: "PATCH",
        body: JSON.stringify(buildAiSettingsPayload({ baseUrl: aiBaseUrl, model: aiModel, apiKey: aiApiKey })),
      });
      setAiBaseUrl(data.settings.baseUrl);
      setAiModel(data.settings.model);
      setAiConfigured(data.settings.configured);
      setAiApiKey("");
      notify("AI 设置已保存", "success");
    } catch (err) {
      notify(`保存 AI 设置失败：${(err as Error).message}`, "error");
    } finally {
      setAiSaving(false);
    }
  };

  const handleTestAiSettings = async () => {
    setAiTesting(true);
    try {
      await api("/api/v1/ai/settings/test", {
        method: "POST",
        body: JSON.stringify(buildAiSettingsPayload({ baseUrl: aiBaseUrl, model: aiModel, apiKey: aiApiKey })),
      });
      notify("AI 连接测试通过", "success");
    } catch (err) {
      notify(`AI 连接测试失败：${(err as Error).message}`, "error");
    } finally {
      setAiTesting(false);
    }
  };

  const resetMigrationDraft = () => {
    setMigrationPreview(null);
    setMigrationResult(null);
    setMigrationProgress(null);
    setOriginalBackupResult(null);
  };

  const handleMigrationBaseUrlChange = (value: string) => {
    setMigrationBaseUrl(value);
    resetMigrationDraft();
  };

  const handleMigrationTokenChange = (value: string) => {
    setMigrationToken(value);
    resetMigrationDraft();
  };

  const handleMigrationIncludeArchivedChange = (value: boolean) => {
    setMigrationIncludeArchived(value);
    resetMigrationDraft();
  };

  const handlePreviewMigration = async () => {
    setMigrationPreviewing(true);
    setMigrationResult(null);
    setMigrationProgress(null);
    try {
      const data = await api<{ preview: MigrationPreview }>("/api/v1/migration/memos/preview", {
        method: "POST",
        body: JSON.stringify(buildMigrationPayload({ baseUrl: migrationBaseUrl, accessToken: migrationToken, includeArchived: migrationIncludeArchived })),
      });
      setMigrationPreview(data.preview);
      notify(`可迁移 ${data.preview.memoCount} 条备忘录`, "success");
    } catch (err) {
      setMigrationPreview(null);
      notify(`预检失败：${(err as Error).message}`, "error");
    } finally {
      setMigrationPreviewing(false);
    }
  };

  const handleRunMigration = async () => {
    const count = migrationPreview?.memoCount;
    const ok = await confirm({
      title: "开始迁移？",
      message: count === undefined
        ? "会从原版 Memos 拉取数据并导入当前账号，重复记录会自动跳过。"
        : `将尝试导入 ${count} 条备忘录，重复记录会自动跳过。`,
      confirmText: "开始迁移",
    });
    if (!ok) return;
    setMigrationImporting(true);
    setMigrationResult(null);
    setMigrationProgress({
      phase: "fetching",
      processed: 0,
      imported: 0,
      skipped: 0,
      memoCount: 0,
      attachmentCount: 0,
      relationCount: 0,
      archivedCount: 0,
      truncated: false,
    });
    try {
      const result = await runMigrationStream("/api/v1/migration/memos/import-stream", buildMigrationPayload({ baseUrl: migrationBaseUrl, accessToken: migrationToken, includeArchived: migrationIncludeArchived }), (progress) => {
        setMigrationProgress(progress);
      });
      setMigrationProgress(result);
      setMigrationResult(result);
      setMigrationPreview(result);
      await fetchTags();
      await refreshAuditLogs();
      await refreshOverview();
      notify(`已导入 ${result.imported} 条，跳过 ${result.skipped} 条`, "success");
    } catch (err) {
      notify(`迁移失败：${(err as Error).message}`, "error");
    } finally {
      setMigrationImporting(false);
    }
  };

  const backupToOriginal = async () => {
    const data = await api<{ result: OriginalBackupResult }>("/api/v1/migration/memos/backup-to-original", {
      method: "POST",
      body: JSON.stringify(buildMigrationPayload({ baseUrl: migrationBaseUrl, accessToken: migrationToken, includeArchived: migrationIncludeArchived })),
    });
    setOriginalBackupResult(data.result);
    await refreshAuditLogs();
    return data.result;
  };

  const handleBackupToOriginal = async () => {
    const ok = await confirm({
      title: "备份到原版 Memos？",
      message: "会把当前账号中尚未推送到该原版 Memos 的备忘录创建过去，不会删除或覆盖原版数据。",
      confirmText: "开始备份",
    });
    if (!ok) return;
    setOriginalBackuping(true);
    setOriginalBackupResult(null);
    try {
      const result = await backupToOriginal();
      notify(`已备份 ${result.pushed} 条，跳过 ${result.skipped} 条`, "success");
    } catch (err) {
      notify(`备份到原版失败：${(err as Error).message}`, "error");
    } finally {
      setOriginalBackuping(false);
    }
  };

  const handleRunMutualBackup = async () => {
    const ok = await confirm({
      title: "开始互相备份？",
      message: "会先从原版 Memos 拉取缺失内容，再把当前系统中本地新增且未备份过的内容创建到原版 Memos。不会删除或覆盖两边已有数据。",
      confirmText: "互相备份",
    });
    if (!ok) return;
    setMigrationImporting(true);
    setOriginalBackuping(true);
    setMigrationResult(null);
    setOriginalBackupResult(null);
    setMigrationProgress({
      phase: "fetching",
      processed: 0,
      imported: 0,
      skipped: 0,
      memoCount: 0,
      attachmentCount: 0,
      relationCount: 0,
      archivedCount: 0,
      truncated: false,
    });
    try {
      const importResult = await runMigrationStream("/api/v1/migration/memos/import-stream", buildMigrationPayload({ baseUrl: migrationBaseUrl, accessToken: migrationToken, includeArchived: migrationIncludeArchived }), (progress) => {
        setMigrationProgress(progress);
      });
      setMigrationProgress(importResult);
      setMigrationResult(importResult);
      setMigrationPreview(importResult);
      await fetchTags();
      await refreshOverview();
      const backupResult = await backupToOriginal();
      notify(`互相备份完成：导入 ${importResult.imported} 条，备份 ${backupResult.pushed} 条`, "success");
    } catch (err) {
      notify(`互相备份失败：${(err as Error).message}`, "error");
    } finally {
      setMigrationImporting(false);
      setOriginalBackuping(false);
    }
  };

  const handleCreateBackup = async () => {
    setBackupCreating(true);
    try {
      const data = await api<{ backup: { key: string; size: number; encrypted: boolean; keyId?: string | null } }>("/api/v1/backups", { method: "POST" });
      await fetchBackups();
      await fetchSystemHealth();
      notify(`备份已创建：${data.backup.key}${data.backup.encrypted ? "（已加密）" : ""}`, "success");
    } catch (err) {
      notify(`创建备份失败：${(err as Error).message}`, "error");
    } finally {
      setBackupCreating(false);
    }
  };

  const handlePreviewBackup = async (backup: BackupItem) => {
    try {
      const data = await api<{ preview: BackupPreview }>("/api/v1/backups/preview", {
        method: "POST",
        body: JSON.stringify({ key: backup.key }),
      });
      setBackupPreview(data.preview);
      setRestoringBackupKey(backup.key);
    } catch (err) {
      notify(`预览备份失败：${(err as Error).message}`, "error");
    }
  };

  const handleRestoreBackup = async () => {
    if (!restoringBackupKey) return;
    const ok = await confirm({
      title: "恢复备份？",
      message: "会按备份内容合并恢复备忘录、附件元数据和引用关系。",
      confirmText: "恢复",
      danger: true,
    });
    if (!ok) return;
    try {
      const data = await api<{ restorePoint?: { backup?: { key?: string } } }>("/api/v1/backups/restore", {
        method: "POST",
        body: JSON.stringify({ key: restoringBackupKey }),
      });
      setBackupPreview(null);
      setRestoringBackupKey("");
      await fetchSystemHealth();
      await refreshAuditLogs();
      notify(`备份已恢复${data.restorePoint?.backup?.key ? `，已创建恢复点 ${data.restorePoint.backup.key}` : ""}`, "success");
    } catch (err) {
      notify(`恢复备份失败：${(err as Error).message}`, "error");
    }
  };

  const handleRebuildMemoIndex = async () => {
    const ok = await confirm({
      title: "重建 Memo 索引？",
      message: "会重新生成搜索和标签索引，不会修改备忘录内容。",
      confirmText: "重建索引",
    });
    if (!ok) return;
    setRebuildingIndex(true);
    try {
      const data = await api<{ rebuilt: number; memoIndex: SystemHealth["memoIndex"] }>("/api/v1/memo-index/rebuild", { method: "POST" });
      setSystemHealth((previous) => previous ? { ...previous, memoIndex: data.memoIndex, status: data.memoIndex.healthy ? "healthy" : "degraded" } : previous);
      await fetchSystemHealth();
      await fetchTags();
      await refreshAuditLogs();
      notify(`已重建 ${data.rebuilt} 条 Memo 索引`, "success");
    } catch (err) {
      notify(`重建索引失败：${(err as Error).message}`, "error");
    } finally {
      setRebuildingIndex(false);
    }
  };

  const handleRebuildRelations = async () => {
    const ok = await confirm({
      title: "全库 AI 关联？",
      message: "会分批扫描当前账号全部正常备忘录，为每篇重建引用关系。已存在的出站引用会按本次结果刷新。",
      confirmText: "开始关联",
    });
    if (!ok) return;
    setRebuildingRelations(true);
    setRelationRebuildProgress(null);
    try {
      let cursor: number | null = 0;
      let totalCreated = 0;
      let totalUpdated = 0;
      let totalSkipped = 0;
      let warnings: string[] = [];
      while (cursor !== null) {
        const payload = {
          cursor,
          accumulatedCreated: totalCreated,
          accumulatedUpdated: totalUpdated,
          accumulatedSkipped: totalSkipped,
        };
        const data: { progress: RelationRebuildProgress } = await api("/api/v1/relations/rebuild", {
          method: "POST",
          body: JSON.stringify(payload),
        });
        totalCreated += data.progress.created;
        totalUpdated += data.progress.updated;
        totalSkipped += data.progress.skipped;
        warnings = [...warnings, ...data.progress.warnings].slice(0, 3);
        const merged = {
          ...data.progress,
          created: totalCreated,
          updated: totalUpdated,
          skipped: totalSkipped,
          warnings,
        };
        setRelationRebuildProgress(merged);
        cursor = data.progress.done ? null : data.progress.nextCursor ?? null;
      }
      await refreshAuditLogs();
      notify(`全库关联完成，写入 ${totalCreated} 条关联`, "success");
    } catch (err) {
      notify(`全库关联失败：${(err as Error).message}`, "error");
    } finally {
      setRebuildingRelations(false);
    }
  };

  const handleRenameTag = async (e: Event) => {
    e.preventDefault();
    setTagSaving(true);
    try {
      const data = await api<{ updated: number }>("/api/v1/tags/rename", {
        method: "POST",
        body: JSON.stringify({ from: tagFrom, to: tagTo }),
      });
      setTagFrom("");
      setTagTo("");
      await fetchTags();
      await refreshAuditLogs();
      notify(`已更新 ${data.updated} 条备忘录`, "success");
    } catch (err) {
      notify(`更新标签失败：${(err as Error).message}`, "error");
    } finally {
      setTagSaving(false);
    }
  };

  const migrationProgressView = useMemo(
    () => buildMigrationProgressView({
      previewing: migrationPreviewing,
      importing: migrationImporting,
      preview: migrationPreview,
      progress: migrationProgress,
    }),
    [migrationImporting, migrationPreview, migrationPreviewing, migrationProgress]
  );

  return {
    backups,
    backupCreating,
    backupPreview,
    aiBaseUrl,
    aiModel,
    aiApiKey,
    aiConfigured,
    aiSaving,
    aiTesting,
    migrationBaseUrl,
    migrationToken,
    migrationIncludeArchived,
    migrationPreview,
    migrationResult,
    migrationProgress,
    originalBackupResult,
    migrationPreviewing,
    migrationImporting,
    originalBackuping,
    tags,
    tagFrom,
    tagTo,
    tagSaving,
    systemHealth,
    healthLoading,
    rebuildingIndex,
    rebuildingRelations,
    relationRebuildProgress,
    migrationProgressVisible: migrationProgressView.visible,
    migrationKnownTotal: migrationProgressView.knownTotal,
    migrationProgressPercent: migrationProgressView.percent,
    migrationProgressTitle: migrationProgressView.title,
    migrationProgressDetail: migrationProgressView.detail,
    onExport: handleExport,
    onImport: handleImport,
    onCreateBackup: handleCreateBackup,
    onPreviewBackup: handlePreviewBackup,
    onRestoreBackup: handleRestoreBackup,
    onRefreshHealth: fetchSystemHealth,
    onRebuildMemoIndex: handleRebuildMemoIndex,
    onRebuildRelations: handleRebuildRelations,
    onAiBaseUrlChange: setAiBaseUrl,
    onAiModelChange: setAiModel,
    onAiApiKeyChange: setAiApiKey,
    onTestAiSettings: handleTestAiSettings,
    onSaveAiSettings: handleSaveAiSettings,
    onMigrationBaseUrlChange: handleMigrationBaseUrlChange,
    onMigrationTokenChange: handleMigrationTokenChange,
    onMigrationIncludeArchivedChange: handleMigrationIncludeArchivedChange,
    onPreviewMigration: handlePreviewMigration,
    onRunMigration: handleRunMigration,
    onBackupToOriginal: handleBackupToOriginal,
    onRunMutualBackup: handleRunMutualBackup,
    onTagFromChange: setTagFrom,
    onTagToChange: setTagTo,
    onRenameTag: handleRenameTag,
  };
}
