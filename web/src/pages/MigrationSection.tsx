import type { MigrationPreview, MigrationProgress, MigrationResult } from "./settingsModel";

interface MigrationSectionProps {
  migrationBaseUrl: string;
  migrationToken: string;
  migrationIncludeArchived: boolean;
  migrationPreview: MigrationPreview | null;
  migrationResult: MigrationResult | null;
  migrationProgress: MigrationProgress | null;
  migrationPreviewing: boolean;
  migrationImporting: boolean;
  migrationProgressVisible: boolean;
  migrationKnownTotal: number;
  migrationProgressPercent: number | null;
  migrationProgressTitle: string;
  migrationProgressDetail: string;
  onMigrationBaseUrlChange: (value: string) => void;
  onMigrationTokenChange: (value: string) => void;
  onMigrationIncludeArchivedChange: (value: boolean) => void;
  onPreviewMigration: () => void;
  onRunMigration: () => void;
}

export function MigrationSection({
  migrationBaseUrl,
  migrationToken,
  migrationIncludeArchived,
  migrationPreview,
  migrationResult,
  migrationProgress,
  migrationPreviewing,
  migrationImporting,
  migrationProgressVisible,
  migrationKnownTotal,
  migrationProgressPercent,
  migrationProgressTitle,
  migrationProgressDetail,
  onMigrationBaseUrlChange,
  onMigrationTokenChange,
  onMigrationIncludeArchivedChange,
  onPreviewMigration,
  onRunMigration,
}: MigrationSectionProps) {
  const migrationBusy = migrationPreviewing || migrationImporting;
  return (
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
  );
}
