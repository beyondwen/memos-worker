import { formatBytes } from "../attachmentCleanupView";
import type { CurrentUser } from "../App";
import { personalSettingsTabs } from "../personalMode";
import {
  SETTINGS_TABS,
  auditLogDetail,
  type Attachment,
  type AuditLog,
  type SettingsTab,
} from "./settingsModel";

interface SettingsTabBarProps {
  currentUser: CurrentUser;
  activeSettingsTab: SettingsTab;
  onChange: (tab: SettingsTab) => void;
}

export function SettingsTabBar({ currentUser, activeSettingsTab, onChange }: SettingsTabBarProps) {
  return (
    <div class="settings-tabs" role="tablist" aria-label="设置分类">
      {personalSettingsTabs(SETTINGS_TABS, currentUser.role).map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={activeSettingsTab === tab.id}
          class={`settings-tab${activeSettingsTab === tab.id ? " active" : ""}`}
          onClick={() => onChange(tab.id)}
        >
          <span>{tab.label}</span>
          <small>{tab.description}</small>
        </button>
      ))}
    </div>
  );
}

interface MaintenanceSettingsTabProps {
  attachmentSummary: { count: number; sizeLabel: string };
  unattachedAttachments: Attachment[];
  deletingAttachmentUid: string;
  onBatchDeleteAttachments: (olderThanDays?: number) => void;
  onDeleteAttachment: (attachment: Attachment) => void;
}

export function MaintenanceSettingsTab({
  attachmentSummary,
  unattachedAttachments,
  deletingAttachmentUid,
  onBatchDeleteAttachments,
  onDeleteAttachment,
}: MaintenanceSettingsTabProps) {
  return (
    <div class="settings-section">
      <h2>附件清理</h2>
      <div class="settings-actions">
        <span class="muted-line">{attachmentSummary.count} 个未绑定附件，共 {attachmentSummary.sizeLabel}</span>
        <button class="btn btn-danger btn-sm" onClick={() => onBatchDeleteAttachments()} disabled={unattachedAttachments.length === 0}>
          全部清理
        </button>
        <button class="btn btn-secondary btn-sm" onClick={() => onBatchDeleteAttachments(30)} disabled={unattachedAttachments.length === 0}>
          清理 30 天前
        </button>
      </div>
      <div class="settings-record-list">
        {unattachedAttachments.map((attachment) => (
          <div key={attachment.uid} class="settings-record-row attachment-cleanup-item">
            <div class="settings-record-main">
              <span class="settings-record-title">{attachment.filename}</span>
              <span class="settings-record-meta">{formatBytes(attachment.size)}</span>
            </div>
            <div class="settings-record-actions">
              <a class="btn btn-ghost btn-sm" href={attachment.url} target="_blank" rel="noopener noreferrer">
                预览
              </a>
              <button
                class="btn btn-danger-soft btn-sm"
                onClick={() => onDeleteAttachment(attachment)}
                disabled={deletingAttachmentUid === attachment.uid}
              >
                {deletingAttachmentUid === attachment.uid ? "删除中..." : "删除"}
              </button>
            </div>
          </div>
        ))}
        {unattachedAttachments.length === 0 && <div class="muted-line">暂无未绑定附件。</div>}
      </div>
    </div>
  );
}

export function AuditSettingsTab({ auditLogs }: { auditLogs: AuditLog[] }) {
  return (
    <div class="settings-section">
      <h2>操作审计</h2>
      <div class="webhook-delivery-list">
        {auditLogs.map((log) => (
          <div key={log.id} class="webhook-delivery-item">
            <div class="webhook-delivery-main">
              <span class="delivery-event">{log.actionLabel}</span>
              <span class="delivery-name">{log.actorUsername ?? "system"}</span>
              <span class="delivery-time">{new Date(log.createdTs * 1000).toLocaleString("zh-CN")}</span>
            </div>
            <span class="delivery-error">{auditLogDetail(log)}</span>
          </div>
        ))}
        {auditLogs.length === 0 && <div class="muted-line">暂无审计记录。</div>}
      </div>
    </div>
  );
}
