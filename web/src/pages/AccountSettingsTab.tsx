import type { CurrentUser } from "../App";
import type { NewPat, Pat, UserStats } from "./settingsModel";

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
