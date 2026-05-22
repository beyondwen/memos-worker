import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { useFeedback } from "../components/Feedback";
import { attachmentCleanupSummary } from "../attachmentCleanupView";
import { PERSONAL_MODE_FEATURES } from "../personalMode";
import type { CurrentUser } from "../App";
import { AccountSettingsTab } from "./AccountSettingsTab";
import { DataSettingsTab } from "./DataSettingsTab";
import {
  type Attachment,
  type AuditLog,
  type SettingsTab,
  type UserStats,
} from "./settingsModel";
import { reportSettingsLoadError } from "./settingsErrors";
import { AuditSettingsTab, MaintenanceSettingsTab, SettingsTabBar } from "./settingsTabs";
import { useSettingsDataController } from "./useSettingsDataController";

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

  const [unattachedAttachments, setUnattachedAttachments] = useState<Attachment[]>([]);
  const [deletingAttachmentUid, setDeletingAttachmentUid] = useState("");
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
    if (
      (currentUser?.role !== "ADMIN" && (activeSettingsTab === "data" || activeSettingsTab === "audit")) ||
      (!PERSONAL_MODE_FEATURES.audit && activeSettingsTab === "audit")
    ) {
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

  const fetchOverview = useCallback(async () => {
    if (!currentUser) return;
    try {
      const [instance, userStats] = await Promise.all([
        api<{ name: string }>("/api/v1/instance"),
        api<{ stats: UserStats }>(`/api/v1/users/${currentUser.username}/stats`),
      ]);
      setInstanceName(instance.name);
      setStats(userStats.stats);
    } catch (err) {
      reportSettingsLoadError("实例概览", err, notify);
    }
  }, [currentUser, notify]);

  const fetchUnattachedAttachments = useCallback(async () => {
    if (!currentUser) return;
    try {
      const data = await api<{ attachments: Attachment[] }>("/api/v1/attachments?unattached=true");
      setUnattachedAttachments(data.attachments);
    } catch (err) {
      reportSettingsLoadError("未绑定附件", err);
    }
  }, [currentUser]);

  const fetchAuditLogs = useCallback(async () => {
    if (!currentUser || currentUser.role !== "ADMIN" || !PERSONAL_MODE_FEATURES.audit) return;
    try {
      const data = await api<{ logs: AuditLog[] }>("/api/v1/audit-logs");
      setAuditLogs(data.logs);
    } catch (err) {
      reportSettingsLoadError("审计日志", err);
    }
  }, [currentUser]);

  useEffect(() => {
    fetchUnattachedAttachments();
    fetchAuditLogs();
    fetchOverview();
  }, [fetchAuditLogs, fetchOverview, fetchUnattachedAttachments]);

  const dataSettings = useSettingsDataController({
    currentUser,
    notify,
    confirm,
    refreshAuditLogs: fetchAuditLogs,
    refreshOverview: fetchOverview,
  });

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

  const attachmentSummary = attachmentCleanupSummary(unattachedAttachments);

  return (
    <div class="settings-layout">
      <div class="home-toolbar page-toolbar">
        <div>
          <div class="home-kicker">Settings</div>
          <h1>设置</h1>
          <p>管理资料、密码和数据</p>
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
          onNicknameChange={setNickname}
          onEmailChange={setEmail}
          onDescriptionChange={setDescription}
          onAvatarUrlChange={setAvatarUrl}
          onCurrentPasswordChange={setCurrentPassword}
          onNewPasswordChange={setNewPassword}
          onProfileSave={handleProfileSave}
          onPasswordChange={handlePasswordChange}
        />
      )}

      {activeSettingsTab === "data" && currentUser.role === "ADMIN" && (
        <DataSettingsTab {...dataSettings} />
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
