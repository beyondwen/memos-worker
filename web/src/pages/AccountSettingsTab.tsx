import type { CurrentUser } from "../App";
import { PERSONAL_MODE_FEATURES } from "../personalMode";
import type { UserSession, UserStats } from "./settingsModel";

interface AccountSettingsTabProps {
  currentUser: CurrentUser;
  instanceName: string;
  stats: UserStats | null;
  sessions: UserSession[];
  revokingSessionId: string;
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
  onNicknameChange: (value: string) => void;
  onEmailChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onAvatarUrlChange: (value: string) => void;
  onCurrentPasswordChange: (value: string) => void;
  onNewPasswordChange: (value: string) => void;
  onProfileSave: (event: Event) => void;
  onPasswordChange: (event: Event) => void;
  onRevokeSession: (session: UserSession) => void;
}

export function AccountSettingsTab({
  currentUser,
  instanceName,
  stats,
  sessions,
  revokingSessionId,
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
  onNicknameChange,
  onEmailChange,
  onDescriptionChange,
  onAvatarUrlChange,
  onCurrentPasswordChange,
  onNewPasswordChange,
  onProfileSave,
  onPasswordChange,
  onRevokeSession,
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
        {PERSONAL_MODE_FEATURES.rss && (
          <div class="settings-links">
            <a href="/api/v1/explore/rss.xml" target="_blank" rel="noopener noreferrer">公开 RSS</a>
            <a href={`/api/v1/u/${encodeURIComponent(currentUser.username)}/rss.xml`} target="_blank" rel="noopener noreferrer">
              我的公开 RSS
            </a>
          </div>
        )}
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
        <h2>登录会话</h2>
        <div class="settings-record-list">
          {sessions.map((session) => (
            <div key={session.id} class="settings-record-row">
              <div class="settings-record-main">
                <span class="settings-record-title">
                  {session.current ? "当前会话" : "其他会话"}
                  {session.rowStatus !== "NORMAL" ? ` · ${session.rowStatus}` : ""}
                </span>
                <span class="settings-record-meta">
                  {session.userAgent || "未知设备"} · 最近 {new Date((session.lastUsedTs || session.updatedTs || session.createdTs) * 1000).toLocaleString("zh-CN")}
                </span>
              </div>
              <div class="settings-record-actions">
                <button
                  class="btn btn-danger-soft btn-sm"
                  onClick={() => onRevokeSession(session)}
                  disabled={revokingSessionId === session.id || session.rowStatus !== "NORMAL"}
                >
                  {revokingSessionId === session.id ? "撤销中..." : "撤销"}
                </button>
              </div>
            </div>
          ))}
          {sessions.length === 0 && <div class="muted-line">暂无会话。</div>}
        </div>
      </div>
    </>
  );
}
