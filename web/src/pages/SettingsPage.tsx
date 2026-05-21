import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import type { CurrentUser } from "../App";

interface SettingsPageProps {
  path: string;
  currentUser: CurrentUser | null;
}

interface Pat {
  id: number;
  name: string;
  prefix: string;
  createdTs: number;
  expiresTs: number | null;
  rowStatus: string;
}

interface NewPat {
  id: number;
  name: string;
  token: string;
  prefix: string;
  createdTs: number;
  expiresTs: number | null;
}

export function SettingsPage({ currentUser }: SettingsPageProps) {
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

  useEffect(() => {
    if (!currentUser) {
      route("/auth", true);
    }
  }, [currentUser]);

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
      alert(`Failed: ${(err as Error).message}`);
    } finally {
      setPatCreating(false);
    }
  };

  const handleDeletePat = async (id: number) => {
    if (!confirm("确定删除此令牌？")) return;
    try {
      await api(
        `/api/v1/users/${currentUser.username}/access-tokens/${id}`,
        { method: "DELETE" }
      );
      fetchPats();
    } catch (err) {
      alert(`Failed: ${(err as Error).message}`);
    }
  };

  const formatTs = (ts: number) =>
    new Date(ts * 1000).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });

  return (
    <div class="settings-layout">
      <h1 style={{ fontSize: "1.4rem", fontWeight: 700 }}>设置</h1>

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
            <div style={{ marginBottom: "12px", fontSize: "0.88rem", color: profileMsg.startsWith("Error") ? "var(--danger)" : "var(--success)" }}>
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
            <div style={{ marginBottom: "12px", fontSize: "0.88rem", color: "var(--success)" }}>
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

        <div class="pat-list">
          {pats.map((pat) => (
            <div key={pat.id} class="pat-item">
              <span class="pat-name">{pat.name}</span>
              <span class="pat-prefix">{pat.prefix}...</span>
              <span class="pat-date">
                {pat.expiresTs ? `过期时间 ${formatTs(pat.expiresTs)}` : "无过期时间"}
              </span>
              <button
                class="btn btn-ghost btn-sm"
                onClick={() => handleDeletePat(pat.id)}
                style={{ color: "var(--danger)" }}
              >
                删除
              </button>
            </div>
          ))}
          {pats.length === 0 && (
            <div style={{ color: "var(--text-muted)", fontSize: "0.88rem" }}>
              暂未创建令牌。
            </div>
          )}
        </div>

        <form onSubmit={handleCreatePat} style={{ display: "flex", gap: "8px", alignItems: "end" }}>
          <div class="form-group" style={{ flex: 1, marginBottom: 0 }}>
            <input
              class="form-input"
              type="text"
              placeholder="令牌名称"
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
            <div style={{ marginBottom: "6px", fontWeight: 600 }}>
              令牌已创建！请立即复制，之后将不再显示。
            </div>
            <code>{newPatResult.token}</code>
          </div>
        )}
      </div>
    </div>
  );
}
