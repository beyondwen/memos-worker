import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { useFeedback } from "../components/Feedback";
import { normalizeWebhookForm } from "../integrationHelpers";
import { webhookDeliveryStatusMeta, webhookDeliveryTimeLabel } from "../webhookDeliveryView";
import { attachmentCleanupSummary, formatBytes } from "../attachmentCleanupView";
import type { CurrentUser } from "../App";
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

  const aiSettingsPayload = () => ({
    baseUrl: aiBaseUrl.trim(),
    model: aiModel.trim(),
    apiKey: aiApiKey.trim(),
  });

  const handleSaveAiSettings = async () => {
    setAiSaving(true);
    try {
      const data = await api<{ settings: AiSettings }>("/api/v1/ai/settings", {
        method: "PATCH",
        body: JSON.stringify(aiSettingsPayload()),
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
        body: JSON.stringify(aiSettingsPayload()),
      });
      notify("AI 连接测试通过", "success");
    } catch (err) {
      notify(`AI 连接测试失败：${(err as Error).message}`, "error");
    } finally {
      setAiTesting(false);
    }
  };

  const migrationPayload = () => ({
    baseUrl: migrationBaseUrl.trim(),
    accessToken: migrationToken.trim(),
    includeArchived: migrationIncludeArchived,
  });

  const handlePreviewMigration = async () => {
    setMigrationPreviewing(true);
    setMigrationResult(null);
    setMigrationProgress(null);
    try {
      const data = await api<{ preview: MigrationPreview }>("/api/v1/migration/memos/preview", {
        method: "POST",
        body: JSON.stringify(migrationPayload()),
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
      const result = await runMigrationStream("/api/v1/migration/memos/import-stream", migrationPayload(), (progress) => {
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

  const formatTs = (ts: number) =>
    new Date(ts * 1000).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });

  const attachmentSummary = attachmentCleanupSummary(unattachedAttachments);
  const migrationBusy = migrationPreviewing || migrationImporting;
  const migrationProgressVisible = migrationBusy || !!migrationProgress;
  const migrationKnownTotal = migrationPreview?.memoCount || 0;
  const migrationProgressPercent = migrationProgress && migrationKnownTotal > 0
    ? Math.min(100, Math.round((migrationProgress.processed / migrationKnownTotal) * 100))
    : null;
  const migrationProgressTitle = migrationPreviewing
    ? "正在预检源数据"
    : migrationProgress?.phase === "done"
      ? "迁移完成"
      : "正在迁移备忘录";
  const migrationProgressDetail = migrationPreviewing
    ? "正在读取原版 Memos 列表和元信息"
    : migrationProgress
      ? `已处理 ${migrationProgress.processed}${migrationKnownTotal ? ` / ${migrationKnownTotal}` : ""} 条，导入 ${migrationProgress.imported} 条，跳过 ${migrationProgress.skipped} 条`
      : "正在拉取并导入，完成后显示导入和跳过数量";

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
        <form onSubmit={handleProfileSave}>
          <div class="form-group">
            <label class="form-label">昵称</label>
            <input
              class="form-input"
              type="text"
              value={nickname}
              onInput={(e) => setNickname((e.target as HTMLInputElement).value)}
            />
          </div>
          <div class="form-group">
            <label class="form-label">邮箱</label>
            <input
              class="form-input"
              type="email"
              value={email}
              onInput={(e) => setEmail((e.target as HTMLInputElement).value)}
            />
          </div>
          <div class="form-group">
            <label class="form-label">简介</label>
            <textarea
              class="form-input"
              value={description}
              onInput={(e) => setDescription((e.target as HTMLTextAreaElement).value)}
              rows={3}
            />
          </div>
          <div class="form-group">
            <label class="form-label">头像链接</label>
            <input
              class="form-input"
              type="text"
              value={avatarUrl}
              onInput={(e) => setAvatarUrl((e.target as HTMLInputElement).value)}
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
        <form onSubmit={handlePasswordChange}>
          <div class="form-group">
            <label class="form-label">当前密码</label>
            <input
              class="form-input"
              type="password"
              value={currentPassword}
              onInput={(e) => setCurrentPassword((e.target as HTMLInputElement).value)}
              autoComplete="current-password"
            />
          </div>
          <div class="form-group">
            <label class="form-label">新密码</label>
            <input
              class="form-input"
              type="password"
              value={newPassword}
              onInput={(e) => setNewPassword((e.target as HTMLInputElement).value)}
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
                  {pat.prefix}... · {pat.expiresTs ? `过期时间 ${formatTs(pat.expiresTs)}` : "无过期时间"}
                </span>
              </div>
              <button
                class="btn btn-danger-soft btn-sm"
                onClick={() => handleDeletePat(pat.id)}
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

        <form onSubmit={handleCreatePat} class="inline-form">
          <div class="form-group">
            <input
              class="form-input"
              type="text"
              placeholder="令牌名称"
              aria-label="令牌名称"
              value={newPatName}
              onInput={(e) => setNewPatName((e.target as HTMLInputElement).value)}
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
      )}

      {activeSettingsTab === "integrations" && (
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
                <button class="btn btn-ghost btn-sm" onClick={() => handleToggleWebhook(webhook)}>
                  {webhook.rowStatus === "NORMAL" ? "停用" : "启用"}
                </button>
                <button
                  class="btn btn-ghost btn-sm"
                  onClick={() => handleTestWebhook(webhook)}
                  disabled={testingWebhookId === webhook.id}
                >
                  {testingWebhookId === webhook.id ? "测试中..." : "测试"}
                </button>
                <button class="btn btn-danger-soft btn-sm" onClick={() => handleDeleteWebhook(webhook)}>
                  删除
                </button>
              </div>
            </div>
          ))}
          {webhooks.length === 0 && <div class="muted-line">暂无 Webhook。</div>}
        </div>
        <form onSubmit={handleCreateWebhook} class="inline-form">
          <div class="form-group">
            <input
              class="form-input"
              type="text"
              placeholder="名称"
              aria-label="Webhook 名称"
              value={webhookName}
              onInput={(e) => setWebhookName((e.target as HTMLInputElement).value)}
            />
          </div>
          <div class="form-group">
            <input
              class="form-input"
              type="url"
              placeholder="https://example.com/webhook"
              aria-label="Webhook 地址"
              value={webhookUrl}
              onInput={(e) => setWebhookUrl((e.target as HTMLInputElement).value)}
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
                        onClick={() => handleRetryWebhookDelivery(delivery)}
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
      )}

      {activeSettingsTab === "data" && currentUser.role === "ADMIN" && (
        <>
        <div class="settings-section">
          <h2>数据维护</h2>
          <div class="settings-actions">
            <button class="btn btn-secondary" onClick={handleExport}>
              导出备忘录
            </button>
            <button class="btn btn-secondary" onClick={handleCreateBackup} disabled={backupCreating}>
              {backupCreating ? "备份中..." : "立即备份"}
            </button>
            <label class="btn btn-secondary file-label">
              导入备忘录
              <input type="file" accept="application/json" onChange={handleImport} />
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
                  <button class="btn btn-ghost btn-sm" onClick={() => handlePreviewBackup(backup)}>
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
              <button class="btn btn-danger btn-sm" onClick={handleRestoreBackup}>恢复此备份</button>
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
                onInput={(e) => setAiBaseUrl((e.target as HTMLInputElement).value)}
                placeholder="https://api.openai.com/v1"
              />
            </div>
            <div class="form-group">
              <label class="form-label">模型</label>
              <input
                class="form-input"
                value={aiModel}
                onInput={(e) => setAiModel((e.target as HTMLInputElement).value)}
                placeholder="gpt-4o-mini"
              />
            </div>
            <div class="form-group ai-key-field">
              <label class="form-label">API Key</label>
              <input
                class="form-input"
                type="password"
                value={aiApiKey}
                onInput={(e) => setAiApiKey((e.target as HTMLInputElement).value)}
                placeholder={aiConfigured ? "已配置，留空则不修改" : "请输入 API Key"}
                autoComplete="off"
              />
            </div>
          </div>
          <div class="settings-actions">
            <button class="btn btn-secondary" onClick={handleTestAiSettings} disabled={aiTesting || !aiBaseUrl.trim() || !aiModel.trim()}>
              {aiTesting ? "测试中..." : "测试连接"}
            </button>
            <button class="btn btn-primary" onClick={handleSaveAiSettings} disabled={aiSaving || !aiBaseUrl.trim() || !aiModel.trim()}>
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
                onInput={(e) => {
                  setMigrationBaseUrl((e.target as HTMLInputElement).value);
                  setMigrationPreview(null);
                  setMigrationResult(null);
                  setMigrationProgress(null);
                }}
              />
            </div>
            <div class="form-group">
              <label class="form-label">Access Token</label>
              <input
                class="form-input"
                type="password"
                placeholder="只用于本次迁移，不会保存"
                value={migrationToken}
                onInput={(e) => {
                  setMigrationToken((e.target as HTMLInputElement).value);
                  setMigrationPreview(null);
                  setMigrationResult(null);
                  setMigrationProgress(null);
                }}
                autoComplete="off"
              />
            </div>
            <label class="migration-option">
              <input
                type="checkbox"
                checked={migrationIncludeArchived}
                onChange={(e) => {
                  setMigrationIncludeArchived((e.target as HTMLInputElement).checked);
                  setMigrationPreview(null);
                  setMigrationResult(null);
                  setMigrationProgress(null);
                }}
              />
              <span>包含归档内容</span>
            </label>
          </div>
          <div class="settings-actions">
            <button
              class="btn btn-secondary"
              onClick={handlePreviewMigration}
              disabled={migrationPreviewing || migrationImporting || !migrationBaseUrl.trim() || !migrationToken.trim()}
            >
              {migrationPreviewing ? "预检中..." : "预检"}
            </button>
            <button
              class="btn btn-primary"
              onClick={handleRunMigration}
              disabled={migrationPreviewing || migrationImporting || !migrationBaseUrl.trim() || !migrationToken.trim()}
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
            <button key={tag.name} class="tag-item" onClick={() => setTagFrom(tag.name)}>
              #{tag.name} <span>{tag.count}</span>
            </button>
          ))}
          {tags.length === 0 && <div class="muted-line">暂无标签。</div>}
        </div>
        <form class="inline-form" onSubmit={handleRenameTag}>
          <input class="form-input" placeholder="原标签" aria-label="原标签" value={tagFrom} onInput={(e) => setTagFrom((e.target as HTMLInputElement).value)} />
          <input class="form-input" placeholder="新标签" aria-label="新标签" value={tagTo} onInput={(e) => setTagTo((e.target as HTMLInputElement).value)} />
          <button class="btn btn-primary btn-sm" disabled={tagSaving || !tagFrom || !tagTo}>
            {tagSaving ? "处理中..." : "重命名/合并"}
          </button>
        </form>
      </div>
        </>
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
