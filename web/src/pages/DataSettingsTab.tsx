import { formatBytes } from "../attachmentCleanupView";
import type {
  BackupItem,
  BackupPreview,
  MigrationPreview,
  MigrationProgress,
  MigrationResult,
  TagItem,
} from "./settingsModel";

interface DataSettingsTabProps {
  backups: BackupItem[];
  backupCreating: boolean;
  backupPreview: BackupPreview | null;
  aiBaseUrl: string;
  aiModel: string;
  aiApiKey: string;
  aiConfigured: boolean;
  aiSaving: boolean;
  aiTesting: boolean;
  migrationBaseUrl: string;
  migrationToken: string;
  migrationIncludeArchived: boolean;
  migrationPreview: MigrationPreview | null;
  migrationResult: MigrationResult | null;
  migrationProgress: MigrationProgress | null;
  migrationPreviewing: boolean;
  migrationImporting: boolean;
  tags: TagItem[];
  tagFrom: string;
  tagTo: string;
  tagSaving: boolean;
  migrationProgressVisible: boolean;
  migrationKnownTotal: number;
  migrationProgressPercent: number | null;
  migrationProgressTitle: string;
  migrationProgressDetail: string;
  onExport: () => void;
  onImport: (event: Event) => void;
  onCreateBackup: () => void;
  onPreviewBackup: (backup: BackupItem) => void;
  onRestoreBackup: () => void;
  onAiBaseUrlChange: (value: string) => void;
  onAiModelChange: (value: string) => void;
  onAiApiKeyChange: (value: string) => void;
  onTestAiSettings: () => void;
  onSaveAiSettings: () => void;
  onMigrationBaseUrlChange: (value: string) => void;
  onMigrationTokenChange: (value: string) => void;
  onMigrationIncludeArchivedChange: (value: boolean) => void;
  onPreviewMigration: () => void;
  onRunMigration: () => void;
  onTagFromChange: (value: string) => void;
  onTagToChange: (value: string) => void;
  onRenameTag: (event: Event) => void;
}

export function DataSettingsTab({
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
  migrationPreviewing,
  migrationImporting,
  tags,
  tagFrom,
  tagTo,
  tagSaving,
  migrationProgressVisible,
  migrationKnownTotal,
  migrationProgressPercent,
  migrationProgressTitle,
  migrationProgressDetail,
  onExport,
  onImport,
  onCreateBackup,
  onPreviewBackup,
  onRestoreBackup,
  onAiBaseUrlChange,
  onAiModelChange,
  onAiApiKeyChange,
  onTestAiSettings,
  onSaveAiSettings,
  onMigrationBaseUrlChange,
  onMigrationTokenChange,
  onMigrationIncludeArchivedChange,
  onPreviewMigration,
  onRunMigration,
  onTagFromChange,
  onTagToChange,
  onRenameTag,
}: DataSettingsTabProps) {
  const migrationBusy = migrationPreviewing || migrationImporting;

  return (
    <>
      <div class="settings-section">
        <h2>数据维护</h2>
        <div class="settings-actions">
          <button class="btn btn-secondary" onClick={onExport}>
            导出备忘录
          </button>
          <button class="btn btn-secondary" onClick={onCreateBackup} disabled={backupCreating}>
            {backupCreating ? "备份中..." : "立即备份"}
          </button>
          <label class="btn btn-secondary file-label">
            导入备忘录
            <input type="file" accept="application/json" onChange={onImport} />
          </label>
        </div>
        <div class="settings-subtitle">备份列表</div>
        <div class="settings-record-list">
          {backups.map((backup) => (
            <div key={backup.key} class="settings-record-row">
              <div class="settings-record-main">
                <span class="settings-record-title">{backup.key.split("/").pop()}</span>
                <span class="settings-record-meta">{formatBytes(backup.size)} · {new Date(backup.uploaded).toLocaleString("zh-CN")}</span>
              </div>
              <div class="settings-record-actions">
                <a class="btn btn-ghost btn-sm" href={`/api/v1/backups/download?key=${encodeURIComponent(backup.key)}`} target="_blank" rel="noopener noreferrer">
                  下载
                </a>
                <button class="btn btn-ghost btn-sm" onClick={() => onPreviewBackup(backup)}>
                  预览
                </button>
              </div>
            </div>
          ))}
          {backups.length === 0 && <div class="muted-line">暂无备份。</div>}
        </div>
        {backupPreview && (
          <div class="backup-preview">
            <span>用户 {backupPreview.userCount}</span>
            <span>备忘录 {backupPreview.memoCount}</span>
            <span>附件 {backupPreview.attachmentCount}</span>
            <span>引用 {backupPreview.relationCount}</span>
            <button class="btn btn-danger btn-sm" onClick={onRestoreBackup}>恢复此备份</button>
          </div>
        )}
      </div>

      <div class="settings-section">
        <h2>AI 设置</h2>
        <div class="ai-settings-grid">
          <div class="form-group">
            <label class="form-label">API Base URL</label>
            <input
              class="form-input"
              type="url"
              value={aiBaseUrl}
              onInput={(e) => onAiBaseUrlChange((e.target as HTMLInputElement).value)}
              placeholder="https://api.openai.com/v1"
            />
          </div>
          <div class="form-group">
            <label class="form-label">模型</label>
            <input
              class="form-input"
              value={aiModel}
              onInput={(e) => onAiModelChange((e.target as HTMLInputElement).value)}
              placeholder="gpt-4o-mini"
            />
          </div>
          <div class="form-group ai-key-field">
            <label class="form-label">API Key</label>
            <input
              class="form-input"
              type="password"
              value={aiApiKey}
              onInput={(e) => onAiApiKeyChange((e.target as HTMLInputElement).value)}
              placeholder={aiConfigured ? "已配置，留空则不修改" : "请输入 API Key"}
              autoComplete="off"
            />
          </div>
        </div>
        <div class="settings-actions">
          <button class="btn btn-secondary" onClick={onTestAiSettings} disabled={aiTesting || !aiBaseUrl.trim() || !aiModel.trim()}>
            {aiTesting ? "测试中..." : "测试连接"}
          </button>
          <button class="btn btn-primary" onClick={onSaveAiSettings} disabled={aiSaving || !aiBaseUrl.trim() || !aiModel.trim()}>
            {aiSaving ? "保存中..." : "保存 AI 设置"}
          </button>
        </div>
        <div class="migration-summary">
          <span>{aiConfigured ? "API Key 已配置" : "API Key 未配置"}</span>
          <span>{aiModel || "未设置模型"}</span>
        </div>
        <div class="muted-line">
          API Key 不会回显；留空保存时会保留已有 Key。
        </div>
      </div>

      <div class="settings-section">
        <h2>从原版 Memos 迁移</h2>
        <div class="migration-form">
          <div class="form-group">
            <label class="form-label">原版 Memos 地址</label>
            <input
              class="form-input"
              type="url"
              placeholder="https://memos.example.com"
              value={migrationBaseUrl}
              onInput={(e) => onMigrationBaseUrlChange((e.target as HTMLInputElement).value)}
            />
          </div>
          <div class="form-group">
            <label class="form-label">Access Token</label>
            <input
              class="form-input"
              type="password"
              placeholder="只用于本次迁移，不会保存"
              value={migrationToken}
              onInput={(e) => onMigrationTokenChange((e.target as HTMLInputElement).value)}
              autoComplete="off"
            />
          </div>
          <label class="migration-option">
            <input
              type="checkbox"
              checked={migrationIncludeArchived}
              onChange={(e) => onMigrationIncludeArchivedChange((e.target as HTMLInputElement).checked)}
            />
            <span>包含归档内容</span>
          </label>
        </div>
        <div class="settings-actions">
          <button
            class="btn btn-secondary"
            onClick={onPreviewMigration}
            disabled={migrationBusy || !migrationBaseUrl.trim() || !migrationToken.trim()}
          >
            {migrationPreviewing ? "预检中..." : "预检"}
          </button>
          <button
            class="btn btn-primary"
            onClick={onRunMigration}
            disabled={migrationBusy || !migrationBaseUrl.trim() || !migrationToken.trim()}
          >
            {migrationImporting ? "迁移中..." : "开始迁移"}
          </button>
        </div>
        {migrationProgressVisible && (
          <div
            class="migration-progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={migrationKnownTotal || undefined}
            aria-valuenow={migrationProgressPercent ?? undefined}
            aria-valuetext={migrationProgressDetail}
          >
            <div class="migration-progress-track" aria-hidden="true">
              <div
                class={`migration-progress-fill${migrationProgressPercent !== null ? " determinate" : ""}`}
                style={migrationProgressPercent !== null ? { width: `${migrationProgressPercent}%` } : undefined}
              />
            </div>
            <div class="migration-progress-text">
              <strong>{migrationProgressTitle}</strong>
              <span>{migrationProgressDetail}</span>
            </div>
            {migrationProgress && (
              <div class="migration-progress-stats">
                <span>已处理 {migrationProgress.processed}</span>
                <span>已导入 {migrationProgress.imported}</span>
                <span>已跳过 {migrationProgress.skipped}</span>
                <span>已读取 {migrationProgress.memoCount}</span>
              </div>
            )}
          </div>
        )}
        {(migrationPreview || migrationResult) && (
          <div class="migration-summary">
            <span>备忘录 {migrationPreview?.memoCount ?? migrationResult?.memoCount}</span>
            <span>归档 {migrationPreview?.archivedCount ?? migrationResult?.archivedCount}</span>
            <span>附件元信息 {migrationPreview?.attachmentCount ?? migrationResult?.attachmentCount}</span>
            <span>引用元信息 {migrationPreview?.relationCount ?? migrationResult?.relationCount}</span>
            {(migrationPreview?.truncated || migrationResult?.truncated) && <span>已达到单次上限</span>}
            {migrationResult && (
              <>
                <span>已导入 {migrationResult.imported}</span>
                <span>已跳过 {migrationResult.skipped}</span>
              </>
            )}
          </div>
        )}
        <div class="muted-line">
          附件文件不会在第一版中下载，只会保留原始附件和引用元信息。
        </div>
      </div>

      <div class="settings-section">
        <h2>标签管理</h2>
        <div class="tag-list settings-tag-list">
          {tags.map((tag) => (
            <button key={tag.name} class="tag-item" onClick={() => onTagFromChange(tag.name)}>
              #{tag.name} <span>{tag.count}</span>
            </button>
          ))}
          {tags.length === 0 && <div class="muted-line">暂无标签。</div>}
        </div>
        <form class="inline-form" onSubmit={onRenameTag}>
          <input class="form-input" placeholder="原标签" aria-label="原标签" value={tagFrom} onInput={(e) => onTagFromChange((e.target as HTMLInputElement).value)} />
          <input class="form-input" placeholder="新标签" aria-label="新标签" value={tagTo} onInput={(e) => onTagToChange((e.target as HTMLInputElement).value)} />
          <button class="btn btn-primary btn-sm" disabled={tagSaving || !tagFrom || !tagTo}>
            {tagSaving ? "处理中..." : "重命名/合并"}
          </button>
        </form>
      </div>
    </>
  );
}
