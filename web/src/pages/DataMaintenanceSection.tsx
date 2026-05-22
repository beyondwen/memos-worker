import { formatBytes } from "../attachmentCleanupView";
import type { BackupItem, BackupPreview } from "./settingsModel";

interface DataMaintenanceSectionProps {
  backups: BackupItem[];
  backupCreating: boolean;
  backupPreview: BackupPreview | null;
  onExport: () => void;
  onImport: (event: Event) => void;
  onCreateBackup: () => void;
  onPreviewBackup: (backup: BackupItem) => void;
  onRestoreBackup: () => void;
}

export function DataMaintenanceSection({
  backups,
  backupCreating,
  backupPreview,
  onExport,
  onImport,
  onCreateBackup,
  onPreviewBackup,
  onRestoreBackup,
}: DataMaintenanceSectionProps) {
  return (
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
              <span class="settings-record-meta">
                {formatBytes(backup.size)} · {new Date(backup.uploaded).toLocaleString("zh-CN")}
                {backup.encrypted ? ` · 加密 ${backup.keyId || ""}` : ""}
              </span>
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
  );
}
