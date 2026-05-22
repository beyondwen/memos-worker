import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { useFeedback } from "../components/Feedback";
import { normalizeWebhookForm } from "../integrationHelpers";
import { attachmentCleanupSummary } from "../attachmentCleanupView";
import type { CurrentUser } from "../App";
import { AccountSettingsTab } from "./AccountSettingsTab";
import { DataSettingsTab } from "./DataSettingsTab";
import { IntegrationSettingsTab } from "./IntegrationSettingsTab";
import {
  type AiSettings,
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
import { runMigrationStream } from "./settingsMigration";
import { buildAiSettingsPayload, buildMigrationPayload, buildMigrationProgressView } from "./settingsPageHelpers";
import { AuditSettingsTab, MaintenanceSettingsTab, SettingsTabBar } from "./settingsTabs";

interface SettingsPageProps {
  path: string;
  currentUser: CurrentUser | null;
}

export function SettingsPage({ currentUser }: SettingsPageProps) {
  const { notify, confirm } = useFeedback();
  const [nickname, setNickname] = useState("");
  const [email, setEmail] = useState("");
  const [description, setDescription] = useState("");
  const [avatarUrl, setAvatarUrl] = useState("");
  const [profileSaving, setProfileSaving] = useState(false);
  const [profileMsg, setProfileMsg] = useState("");

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [pwSaving, setPwSaving] = useState(false);
  const [pwMsg, setPwMsg] = useState("");
  const [pwError, setPwError] = useState("");

  const [pats, setPats] = useState<Pat[]>([]);
  const [newPatName, setNewPatName] = useState("");
  const [newPatResult, setNewPatResult] = useState<NewPat | null>(null);
  const [patCreating, setPatCreating] = useState(false);
  const [webhooks, setWebhooks] = useState<Webhook[]>([]);
  const [webhookName, setWebhookName] = useState("");
  const [webhookUrl, setWebhookUrl] = useState("");
  const [webhookSaving, setWebhookSaving] = useState(false);
  const [webhookDeliveries, setWebhookDeliveries] = useState<WebhookDelivery[]>([]);
  const [retryingDeliveryId, setRetryingDeliveryId] = useState<number | null>(null);
  const [testingWebhookId, setTestingWebhookId] = useState<number | null>(null);
  const [unattachedAttachments, setUnattachedAttachments] = useState<Attachment[]>([]);
  const [deletingAttachmentUid, setDeletingAttachmentUid] = useState("");
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
  const [migrationPreviewing, setMigrationPreviewing] = useState(false);
  const [migrationImporting, setMigrationImporting] = useState(false);
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
  const [auditLogs, setAuditLogs] = useState<AuditLog[]>([]);
  const [stats, setStats] = useState<UserStats | null>(null);
  const [instanceName, setInstanceName] = useState("Memos Worker");
  const [activeSettingsTab, setActiveSettingsTab] = useState<SettingsTab>("account");

  useEffect(() => {
    if (!currentUser) {
      route("/auth", true);
    }
  }, [currentUser]);

  useEffect(() => {
    if (currentUser?.role !== "ADMIN" && (activeSettingsTab === "data" || activeSettingsTab === "audit")) {
      setActiveSettingsTab("account");
    }
  }, [activeSettingsTab, currentUser?.role]);

  useEffect(() => {
    if (currentUser) {
      setNickname(currentUser.nickname || "");
      setEmail(currentUser.email || "");
      setDescription(currentUser.description || "");
      setAvatarUrl(currentUser.avatarUrl || "");
    }
  }, [currentUser]);

  const fetchPats = useCallback(async () => {
    if (!currentUser) return;
    try {
      const data = await api<{ accessTokens: Pat[] }>(
        `/api/v1/users/${currentUser.username}/access-tokens`
      );
      setPats(data.accessTokens);
    } catch {
      // ignore
    }
  }, [currentUser]);

  useEffect(() => {
    fetchPats();
  }, [fetchPats]);

  const fetchWebhooks = useCallback(async () => {
    if (!currentUser) return;
    try {
      const data = await api<{ webhooks: Webhook[] }>("/api/v1/webhooks");
      setWebhooks(data.webhooks);
    } catch {
      // ignore
    }
  }, [currentUser]);

  const fetchWebhookDeliveries = useCallback(async () => {
    if (!currentUser) return;
    try {
      const data = await api<{ deliveries: WebhookDelivery[] }>("/api/v1/webhooks/deliveries");
      setWebhookDeliveries(data.deliveries);
    } catch {
      // ignore
    }
  }, [currentUser]);

  const fetchOverview = useCallback(async () => {
    if (!currentUser) return;
    try {
      const [instance, userStats] = await Promise.all([
        api<{ name: string }>("/api/v1/instance"),
        api<{ stats: UserStats }>(`/api/v1/users/${currentUser.username}/stats`),
      ]);
      setInstanceName(instance.name);
      setStats(userStats.stats);
    } catch {
      // ignore
    }
  }, [currentUser]);

  const fetchUnattachedAttachments = useCallback(async () => {
    if (!currentUser) return;
    try {
      const data = await api<{ attachments: Attachment[] }>("/api/v1/attachments?unattached=true");
      setUnattachedAttachments(data.attachments);
    } catch {
      // ignore
    }
  }, [currentUser]);

  const fetchBackups = useCallback(async () => {
    if (!currentUser || currentUser.role !== "ADMIN") return;
    try {
      const data = await api<{ backups: BackupItem[] }>("/api/v1/backups");
      setBackups(data.backups);
    } catch {
      // ignore
    }
  }, [currentUser]);

  const fetchAiSettings = useCallback(async () => {
    if (!currentUser || currentUser.role !== "ADMIN") return;
    try {
      const data = await api<{ settings: AiSettings }>("/api/v1/ai/settings");
      setAiBaseUrl(data.settings.baseUrl);
      setAiModel(data.settings.model);
      setAiConfigured(data.settings.configured);
      setAiApiKey("");
    } catch {
      // ignore
    }
  }, [currentUser]);

  const fetchTags = useCallback(async () => {
    if (!currentUser) return;
    try {
      const data = await api<{ tags: TagItem[] }>("/api/v1/tags");
      setTags(data.tags);
    } catch {
      // ignore
    }
  }, [currentUser]);

  const fetchAuditLogs = useCallback(async () => {
    if (!currentUser || currentUser.role !== "ADMIN") return;
    try {
      const data = await api<{ logs: AuditLog[] }>("/api/v1/audit-logs");
      setAuditLogs(data.logs);
    } catch {
      // ignore
    }
  }, [currentUser]);

  useEffect(() => {
    fetchWebhooks();
    fetchWebhookDeliveries();
    fetchUnattachedAttachments();
    fetchBackups();
    fetchAiSettings();
    fetchTags();
    fetchAuditLogs();
    fetchOverview();
  }, [fetchAiSettings, fetchAuditLogs, fetchBackups, fetchOverview, fetchTags, fetchUnattachedAttachments, fetchWebhookDeliveries, fetchWebhooks]);

  if (!currentUser) return null;

  const handleProfileSave = async (e: Event) => {
    e.preventDefault();
    setProfileSaving(true);
    setProfileMsg("");
    try {
      await api("/api/v1/users/me", {
        method: "PATCH",
        body: JSON.stringify({ nickname, email, description, avatarUrl }),
      });
      setProfileMsg("资料已更新。");
      notify("资料已保存", "success");
    } catch (err) {
      setProfileMsg(`Error: ${(err as Error).message}`);
    } finally {
      setProfileSaving(false);
    }
  };

  const handlePasswordChange = async (e: Event) => {
    e.preventDefault();
    setPwSaving(true);
    setPwMsg("");
    setPwError("");
    try {
      await api("/api/v1/auth/change-password", {
        method: "POST",
        body: JSON.stringify({ currentPassword, newPassword }),
      });
      setPwMsg("密码已修改，请重新登录。");
      setCurrentPassword("");
      setNewPassword("");
    } catch (err) {
      setPwError((err as Error).message);
    } finally {
      setPwSaving(false);
    }
  };

  const handleCreatePat = async (e: Event) => {
    e.preventDefault();
    setPatCreating(true);
    setNewPatResult(null);
    try {
      const data = await api<{ accessToken: NewPat }>(
        `/api/v1/users/${currentUser.username}/access-tokens`,
        {
          method: "POST",
          body: JSON.stringify({ name: newPatName || "Unnamed Token" }),
        }
      );
      setNewPatResult(data.accessToken);
      setNewPatName("");
      fetchPats();
    } catch (err) {
      notify(`创建令牌失败：${(err as Error).message}`, "error");
    } finally {
      setPatCreating(false);
    }
  };

  const handleDeletePat = async (id: number) => {
    const ok = await confirm({
      title: "删除此令牌？",
      message: "删除后使用该令牌的客户端会立即失效。",
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await api(
        `/api/v1/users/${currentUser.username}/access-tokens/${id}`,
        { method: "DELETE" }
      );
      fetchPats();
      notify("令牌已删除", "success");
    } catch (err) {
      notify(`删除令牌失败：${(err as Error).message}`, "error");
    }
  };

  const handleCreateWebhook = async (e: Event) => {
    e.preventDefault();
    const normalized = normalizeWebhookForm(webhookName, webhookUrl);
    if (!normalized.ok) {
      notify(normalized.error, "error");
      return;
    }
    setWebhookSaving(true);
    try {
      await api("/api/v1/webhooks", {
        method: "POST",
        body: JSON.stringify({ name: normalized.name, url: normalized.url }),
      });
      setWebhookName("");
      setWebhookUrl("");
      fetchWebhooks();
      fetchWebhookDeliveries();
      notify("Webhook 已创建", "success");
    } catch (err) {
      notify(`创建 Webhook 失败：${(err as Error).message}`, "error");
    } finally {
      setWebhookSaving(false);
    }
  };

  const handleToggleWebhook = async (webhook: Webhook) => {
    const next = webhook.rowStatus === "NORMAL" ? "ARCHIVED" : "NORMAL";
    await api(`/api/v1/webhooks/${webhook.id}`, {
      method: "PATCH",
      body: JSON.stringify({ rowStatus: next }),
    });
    fetchWebhooks();
    fetchWebhookDeliveries();
  };

  const handleTestWebhook = async (webhook: Webhook) => {
    setTestingWebhookId(webhook.id);
    try {
      await api(`/api/v1/webhooks/${webhook.id}/test`, { method: "POST" });
      await fetchWebhookDeliveries();
      notify("测试事件已发送", "success");
    } catch (err) {
      notify(`测试失败：${(err as Error).message}`, "error");
    } finally {
      setTestingWebhookId(null);
    }
  };

  const handleDeleteWebhook = async (webhook: Webhook) => {
    const ok = await confirm({
      title: "删除 Webhook？",
      message: "删除后相关自动化推送会停止。",
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    await api(`/api/v1/webhooks/${webhook.id}`, { method: "DELETE" });
    fetchWebhooks();
    fetchWebhookDeliveries();
  };

  const handleRetryWebhookDelivery = async (delivery: WebhookDelivery) => {
    setRetryingDeliveryId(delivery.id);
    try {
      await api(`/api/v1/webhooks/deliveries/${delivery.id}/retry`, { method: "POST" });
      await fetchWebhookDeliveries();
      notify("Webhook 已重试", "success");
    } catch (err) {
      notify(`重试失败：${(err as Error).message}`, "error");
    } finally {
      setRetryingDeliveryId(null);
    }
  };

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
      await fetchAuditLogs();
      await fetchOverview();
      notify(`已导入 ${result.imported} 条，跳过 ${result.skipped} 条`, "success");
    } catch (err) {
      notify(`迁移失败：${(err as Error).message}`, "error");
    } finally {
      setMigrationImporting(false);
    }
  };

  const handleDeleteAttachment = async (attachment: Attachment) => {
    const ok = await confirm({
      title: "删除未绑定附件？",
      message: attachment.filename,
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    setDeletingAttachmentUid(attachment.uid);
    try {
      await api(`/api/v1/attachments/${attachment.uid}`, { method: "DELETE" });
      await fetchUnattachedAttachments();
      notify("附件已删除", "success");
    } catch (err) {
      notify(`删除附件失败：${(err as Error).message}`, "error");
    } finally {
      setDeletingAttachmentUid("");
    }
  };

  const handleCreateBackup = async () => {
    setBackupCreating(true);
    try {
      const data = await api<{ backup: { key: string; size: number } }>("/api/v1/backups", { method: "POST" });
      await fetchBackups();
      notify(`备份已创建：${data.backup.key}`, "success");
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
      await api("/api/v1/backups/restore", {
        method: "POST",
        body: JSON.stringify({ key: restoringBackupKey }),
      });
      setBackupPreview(null);
      setRestoringBackupKey("");
      await fetchAuditLogs();
      notify("备份已恢复", "success");
    } catch (err) {
      notify(`恢复备份失败：${(err as Error).message}`, "error");
    }
  };

  const handleBatchDeleteAttachments = async (olderThanDays?: number) => {
    const summary = attachmentCleanupSummary(unattachedAttachments);
    const ok = await confirm({
      title: "批量删除未绑定附件？",
      message: `${summary.count} 个附件，共 ${summary.sizeLabel}`,
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await api("/api/v1/attachments/batch-delete", {
        method: "POST",
        body: JSON.stringify({ attachmentUids: unattachedAttachments.map((item) => item.uid), olderThanDays }),
      });
      await fetchUnattachedAttachments();
      await fetchAuditLogs();
      notify("未绑定附件已清理", "success");
    } catch (err) {
      notify(`批量删除失败：${(err as Error).message}`, "error");
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
      await fetchAuditLogs();
      notify(`已更新 ${data.updated} 条备忘录`, "success");
    } catch (err) {
      notify(`更新标签失败：${(err as Error).message}`, "error");
    } finally {
      setTagSaving(false);
    }
  };

  const resetMigrationDraft = () => {
    setMigrationPreview(null);
    setMigrationResult(null);
    setMigrationProgress(null);
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

  const attachmentSummary = attachmentCleanupSummary(unattachedAttachments);
  const migrationProgressView = buildMigrationProgressView({
    previewing: migrationPreviewing,
    importing: migrationImporting,
    preview: migrationPreview,
    progress: migrationProgress,
  });

  return (
    <div class="settings-layout">
      <div class="home-toolbar page-toolbar">
        <div>
          <div class="home-kicker">Settings</div>
          <h1>设置</h1>
          <p>管理资料、密码、集成和数据</p>
        </div>
      </div>

      <SettingsTabBar
        currentUser={currentUser}
        activeSettingsTab={activeSettingsTab}
        onChange={setActiveSettingsTab}
      />

      {activeSettingsTab === "account" && (
        <AccountSettingsTab
          currentUser={currentUser}
          instanceName={instanceName}
          stats={stats}
          nickname={nickname}
          email={email}
          description={description}
          avatarUrl={avatarUrl}
          profileMsg={profileMsg}
          profileSaving={profileSaving}
          currentPassword={currentPassword}
          newPassword={newPassword}
          pwSaving={pwSaving}
          pwMsg={pwMsg}
          pwError={pwError}
          pats={pats}
          newPatName={newPatName}
          newPatResult={newPatResult}
          patCreating={patCreating}
          onNicknameChange={setNickname}
          onEmailChange={setEmail}
          onDescriptionChange={setDescription}
          onAvatarUrlChange={setAvatarUrl}
          onCurrentPasswordChange={setCurrentPassword}
          onNewPasswordChange={setNewPassword}
          onNewPatNameChange={setNewPatName}
          onProfileSave={handleProfileSave}
          onPasswordChange={handlePasswordChange}
          onCreatePat={handleCreatePat}
          onDeletePat={handleDeletePat}
        />
      )}

      {activeSettingsTab === "integrations" && (
        <IntegrationSettingsTab
          webhooks={webhooks}
          webhookName={webhookName}
          webhookUrl={webhookUrl}
          webhookSaving={webhookSaving}
          webhookDeliveries={webhookDeliveries}
          retryingDeliveryId={retryingDeliveryId}
          testingWebhookId={testingWebhookId}
          onWebhookNameChange={setWebhookName}
          onWebhookUrlChange={setWebhookUrl}
          onCreateWebhook={handleCreateWebhook}
          onToggleWebhook={handleToggleWebhook}
          onTestWebhook={handleTestWebhook}
          onDeleteWebhook={handleDeleteWebhook}
          onRetryWebhookDelivery={handleRetryWebhookDelivery}
        />
      )}

      {activeSettingsTab === "data" && currentUser.role === "ADMIN" && (
        <DataSettingsTab
          backups={backups}
          backupCreating={backupCreating}
          backupPreview={backupPreview}
          aiBaseUrl={aiBaseUrl}
          aiModel={aiModel}
          aiApiKey={aiApiKey}
          aiConfigured={aiConfigured}
          aiSaving={aiSaving}
          aiTesting={aiTesting}
          migrationBaseUrl={migrationBaseUrl}
          migrationToken={migrationToken}
          migrationIncludeArchived={migrationIncludeArchived}
          migrationPreview={migrationPreview}
          migrationResult={migrationResult}
          migrationProgress={migrationProgress}
          migrationPreviewing={migrationPreviewing}
          migrationImporting={migrationImporting}
          tags={tags}
          tagFrom={tagFrom}
          tagTo={tagTo}
          tagSaving={tagSaving}
          migrationProgressVisible={migrationProgressView.visible}
          migrationKnownTotal={migrationProgressView.knownTotal}
          migrationProgressPercent={migrationProgressView.percent}
          migrationProgressTitle={migrationProgressView.title}
          migrationProgressDetail={migrationProgressView.detail}
          onExport={handleExport}
          onImport={handleImport}
          onCreateBackup={handleCreateBackup}
          onPreviewBackup={handlePreviewBackup}
          onRestoreBackup={handleRestoreBackup}
          onAiBaseUrlChange={setAiBaseUrl}
          onAiModelChange={setAiModel}
          onAiApiKeyChange={setAiApiKey}
          onTestAiSettings={handleTestAiSettings}
          onSaveAiSettings={handleSaveAiSettings}
          onMigrationBaseUrlChange={handleMigrationBaseUrlChange}
          onMigrationTokenChange={handleMigrationTokenChange}
          onMigrationIncludeArchivedChange={handleMigrationIncludeArchivedChange}
          onPreviewMigration={handlePreviewMigration}
          onRunMigration={handleRunMigration}
          onTagFromChange={setTagFrom}
          onTagToChange={setTagTo}
          onRenameTag={handleRenameTag}
        />
      )}

      {activeSettingsTab === "maintenance" && (
        <MaintenanceSettingsTab
          attachmentSummary={attachmentSummary}
          unattachedAttachments={unattachedAttachments}
          deletingAttachmentUid={deletingAttachmentUid}
          onBatchDeleteAttachments={handleBatchDeleteAttachments}
          onDeleteAttachment={handleDeleteAttachment}
        />
      )}

      {activeSettingsTab === "audit" && currentUser.role === "ADMIN" && (
        <AuditSettingsTab auditLogs={auditLogs} />
      )}
    </div>
  );
}
