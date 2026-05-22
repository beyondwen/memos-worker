import { formatBytes } from "../attachmentCleanupView";
import type { CurrentUser } from "../App";
import { webhookDeliveryStatusMeta, webhookDeliveryTimeLabel } from "../webhookDeliveryView";
import {
  SETTINGS_TABS,
  auditLogDetail,
  type Attachment,
  type AuditLog,
  type BackupItem,
  type BackupPreview,
  type MigrationPreview,
  type MigrationProgress,
  type MigrationResult,
  type NewPat,
  type Pat,
  type SettingsTab,
  type TagItem,
  type UserStats,
  type Webhook,
  type WebhookDelivery,
} from "./settingsModel";

interface SettingsTabBarProps {
  currentUser: CurrentUser;
  activeSettingsTab: SettingsTab;
  onChange: (tab: SettingsTab) => void;
}

export function SettingsTabBar({ currentUser, activeSettingsTab, onChange }: SettingsTabBarProps) {
  return (
    <div class="settings-tabs" role="tablist" aria-label="设置分类">
      {SETTINGS_TABS.filter((tab) => !tab.adminOnly || currentUser.role === "ADMIN").map((tab) => (
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

interface AccountSettingsTabProps {
  currentUser: CurrentUser;
  instanceName: string;
  stats: UserStats | null;
  nickname: string;
  email: string;
  description: string;
  avatarUrl: string;
  profileMsg: string;
  profileSaving: boolean;
  currentPassword: string;
  newPassword: string;
  pwSaving: boolean;
  pwMsg: string;
  pwError: string;
  pats: Pat[];
  newPatName: string;
  newPatResult: NewPat | null;
  patCreating: boolean;
  onNicknameChange: (value: string) => void;
  onEmailChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onAvatarUrlChange: (value: string) => void;
  onCurrentPasswordChange: (value: string) => void;
  onNewPasswordChange: (value: string) => void;
  onNewPatNameChange: (value: string) => void;
  onProfileSave: (event: Event) => void;
  onPasswordChange: (event: Event) => void;
  onCreatePat: (event: Event) => void;
  onDeletePat: (id: number) => void;
}

const formatSettingsDate = (ts: number) =>
  new Date(ts * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });

export function AccountSettingsTab({
  currentUser,
  instanceName,
  stats,
  nickname,
  email,
  description,
  avatarUrl,
  profileMsg,
  profileSaving,
  currentPassword,
  newPassword,
  pwSaving,
  pwMsg,
  pwError,
  pats,
  newPatName,
  newPatResult,
  patCreating,
  onNicknameChange,
  onEmailChange,
  onDescriptionChange,
  onAvatarUrlChange,
  onCurrentPasswordChange,
  onNewPasswordChange,
  onNewPatNameChange,
  onProfileSave,
  onPasswordChange,
  onCreatePat,
  onDeletePat,
}: AccountSettingsTabProps) {
  return (
    <>
      <div class="settings-section">
        <h2>实例概览</h2>
        <div class="overview-grid">
          <div>
            <span class="overview-label">实例</span>
            <strong>{instanceName}</strong>
          </div>
          <div>
            <span class="overview-label">备忘录</span>
            <strong>{stats?.memoCount ?? "-"}</strong>
          </div>
          <div>
            <span class="overview-label">附件</span>
            <strong>{stats?.attachmentCount ?? "-"}</strong>
          </div>
        </div>
        <div class="settings-links">
          <a href="/api/v1/explore/rss.xml" target="_blank" rel="noopener noreferrer">公开 RSS</a>
          <a href={`/api/v1/u/${encodeURIComponent(currentUser.username)}/rss.xml`} target="_blank" rel="noopener noreferrer">
            我的公开 RSS
          </a>
        </div>
      </div>

      <div class="settings-section">
        <h2>个人资料</h2>
        <form onSubmit={onProfileSave}>
          <div class="form-group">
            <label class="form-label">昵称</label>
            <input
              class="form-input"
              type="text"
              value={nickname}
              onInput={(e) => onNicknameChange((e.target as HTMLInputElement).value)}
            />
          </div>
          <div class="form-group">
            <label class="form-label">邮箱</label>
            <input
              class="form-input"
              type="email"
              value={email}
              onInput={(e) => onEmailChange((e.target as HTMLInputElement).value)}
            />
          </div>
          <div class="form-group">
            <label class="form-label">简介</label>
            <textarea
              class="form-input"
              value={description}
              onInput={(e) => onDescriptionChange((e.target as HTMLTextAreaElement).value)}
              rows={3}
            />
          </div>
          <div class="form-group">
            <label class="form-label">头像链接</label>
            <input
              class="form-input"
              type="text"
              value={avatarUrl}
              onInput={(e) => onAvatarUrlChange((e.target as HTMLInputElement).value)}
            />
          </div>
          {profileMsg && (
            <div class={`inline-message ${profileMsg.startsWith("Error") ? "error" : "success"}`}>
              {profileMsg}
            </div>
          )}
          <button class="btn btn-primary" type="submit" disabled={profileSaving}>
            {profileSaving ? "保存中..." : "保存资料"}
          </button>
        </form>
      </div>

      <div class="settings-section">
        <h2>修改密码</h2>
        <form onSubmit={onPasswordChange}>
          <div class="form-group">
            <label class="form-label">当前密码</label>
            <input
              class="form-input"
              type="password"
              value={currentPassword}
              onInput={(e) => onCurrentPasswordChange((e.target as HTMLInputElement).value)}
              autoComplete="current-password"
            />
          </div>
          <div class="form-group">
            <label class="form-label">新密码</label>
            <input
              class="form-input"
              type="password"
              value={newPassword}
              onInput={(e) => onNewPasswordChange((e.target as HTMLInputElement).value)}
              autoComplete="new-password"
            />
          </div>
          {pwError && <div class="form-error">{pwError}</div>}
          {pwMsg && (
            <div class="inline-message success">
              {pwMsg}
            </div>
          )}
          <button
            class="btn btn-primary"
            type="submit"
            disabled={pwSaving || !currentPassword || !newPassword}
          >
            {pwSaving ? "修改中..." : "修改密码"}
          </button>
        </form>
      </div>

      <div class="settings-section">
        <h2>个人访问令牌</h2>

        <div class="settings-record-list">
          {pats.map((pat) => (
            <div key={pat.id} class="settings-record-row">
              <div class="settings-record-main">
                <span class="settings-record-title">{pat.name}</span>
                <span class="settings-record-meta">
                  {pat.prefix}... · {pat.expiresTs ? `过期时间 ${formatSettingsDate(pat.expiresTs)}` : "无过期时间"}
                </span>
              </div>
              <button
                class="btn btn-danger-soft btn-sm"
                onClick={() => onDeletePat(pat.id)}
              >
                删除
              </button>
            </div>
          ))}
          {pats.length === 0 && (
            <div class="muted-line">
              暂未创建令牌。
            </div>
          )}
        </div>

        <form onSubmit={onCreatePat} class="inline-form">
          <div class="form-group">
            <input
              class="form-input"
              type="text"
              placeholder="令牌名称"
              aria-label="令牌名称"
              value={newPatName}
              onInput={(e) => onNewPatNameChange((e.target as HTMLInputElement).value)}
            />
          </div>
          <button class="btn btn-primary btn-sm" type="submit" disabled={patCreating}>
            {patCreating ? "创建中..." : "创建令牌"}
          </button>
        </form>

        {newPatResult && (
          <div class="pat-token-box">
            <div class="pat-token-title">
              令牌已创建！请立即复制，之后将不再显示。
            </div>
            <code>{newPatResult.token}</code>
          </div>
        )}
      </div>
    </>
  );
}

interface IntegrationSettingsTabProps {
  webhooks: Webhook[];
  webhookName: string;
  webhookUrl: string;
  webhookSaving: boolean;
  webhookDeliveries: WebhookDelivery[];
  retryingDeliveryId: number | null;
  testingWebhookId: number | null;
  onWebhookNameChange: (value: string) => void;
  onWebhookUrlChange: (value: string) => void;
  onCreateWebhook: (event: Event) => void;
  onToggleWebhook: (webhook: Webhook) => void;
  onTestWebhook: (webhook: Webhook) => void;
  onDeleteWebhook: (webhook: Webhook) => void;
  onRetryWebhookDelivery: (delivery: WebhookDelivery) => void;
}

export function IntegrationSettingsTab({
  webhooks,
  webhookName,
  webhookUrl,
  webhookSaving,
  webhookDeliveries,
  retryingDeliveryId,
  testingWebhookId,
  onWebhookNameChange,
  onWebhookUrlChange,
  onCreateWebhook,
  onToggleWebhook,
  onTestWebhook,
  onDeleteWebhook,
  onRetryWebhookDelivery,
}: IntegrationSettingsTabProps) {
  return (
    <div class="settings-section">
      <h2>Webhook 集成</h2>
      <div class="settings-record-list">
        {webhooks.map((webhook) => (
          <div key={webhook.id} class="settings-record-row">
            <div class="settings-record-main">
              <span class="settings-record-title">{webhook.name}</span>
              <span class="settings-record-meta">{webhook.rowStatus === "NORMAL" ? "启用" : "停用"} · {webhook.url}</span>
            </div>
            <div class="settings-record-actions">
              <button class="btn btn-ghost btn-sm" onClick={() => onToggleWebhook(webhook)}>
                {webhook.rowStatus === "NORMAL" ? "停用" : "启用"}
              </button>
              <button
                class="btn btn-ghost btn-sm"
                onClick={() => onTestWebhook(webhook)}
                disabled={testingWebhookId === webhook.id}
              >
                {testingWebhookId === webhook.id ? "测试中..." : "测试"}
              </button>
              <button class="btn btn-danger-soft btn-sm" onClick={() => onDeleteWebhook(webhook)}>
                删除
              </button>
            </div>
          </div>
        ))}
        {webhooks.length === 0 && <div class="muted-line">暂无 Webhook。</div>}
      </div>
      <form onSubmit={onCreateWebhook} class="inline-form">
        <div class="form-group">
          <input
            class="form-input"
            type="text"
            placeholder="名称"
            aria-label="Webhook 名称"
            value={webhookName}
            onInput={(e) => onWebhookNameChange((e.target as HTMLInputElement).value)}
          />
        </div>
        <div class="form-group">
          <input
            class="form-input"
            type="url"
            placeholder="https://example.com/webhook"
            aria-label="Webhook 地址"
            value={webhookUrl}
            onInput={(e) => onWebhookUrlChange((e.target as HTMLInputElement).value)}
          />
        </div>
        <button class="btn btn-primary btn-sm" type="submit" disabled={webhookSaving}>
          {webhookSaving ? "创建中..." : "创建"}
        </button>
      </form>

      <div class="webhook-delivery-panel">
        <div class="settings-subtitle">最近投递</div>
        <div class="webhook-delivery-list">
          {webhookDeliveries.map((delivery) => {
            const meta = webhookDeliveryStatusMeta(delivery);
            return (
              <div key={delivery.id} class="webhook-delivery-item">
                <div class="webhook-delivery-main">
                  <span class={`delivery-status ${meta.className}`}>{meta.label}</span>
                  <span class="delivery-event">{delivery.event}</span>
                  <span class="delivery-name">{delivery.webhookName}</span>
                  <span class="delivery-time">{webhookDeliveryTimeLabel(delivery.createdTs)}</span>
                </div>
                <div class="webhook-delivery-meta">
                  <span>{delivery.durationMs}ms</span>
                  {delivery.error && <span class="delivery-error">{delivery.error}</span>}
                  {meta.canRetry && (
                    <button
                      class="btn btn-ghost btn-sm"
                      onClick={() => onRetryWebhookDelivery(delivery)}
                      disabled={retryingDeliveryId === delivery.id}
                    >
                      {retryingDeliveryId === delivery.id ? "重试中..." : "重试"}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
          {webhookDeliveries.length === 0 && <div class="muted-line">暂无投递记录。</div>}
        </div>
      </div>
    </div>
  );
}

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
