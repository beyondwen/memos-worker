import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { useFeedback } from "../components/Feedback";
import { normalizeWebhookForm } from "../integrationHelpers";
import { webhookDeliveryStatusMeta, webhookDeliveryTimeLabel } from "../webhookDeliveryView";
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

interface Webhook {
  id: number;
  name: string;
  url: string;
  rowStatus: "NORMAL" | "ARCHIVED";
  createdTs: number;
  updatedTs: number;
}

interface WebhookDelivery {
  id: number;
  webhookId: number;
  webhookName: string;
  webhookUrl: string;
  createdTs: number;
  event: string;
  status: "SUCCESS" | "FAILED";
  statusCode: number | null;
  durationMs: number;
  error: string;
  responseBody: string;
}

interface UserStats {
  memoCount: number;
  attachmentCount: number;
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
  const [stats, setStats] = useState<UserStats | null>(null);
  const [instanceName, setInstanceName] = useState("Memos Worker");

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

  useEffect(() => {
    fetchWebhooks();
    fetchWebhookDeliveries();
    fetchOverview();
  }, [fetchOverview, fetchWebhookDeliveries, fetchWebhooks]);

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

  const formatTs = (ts: number) =>
    new Date(ts * 1000).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
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

      <div class="settings-section">
        <h2>Webhook 集成</h2>
        <div class="pat-list">
          {webhooks.map((webhook) => (
            <div key={webhook.id} class="pat-item">
              <span class="pat-name">{webhook.name}</span>
              <span class="pat-prefix">{webhook.rowStatus === "NORMAL" ? "启用" : "停用"}</span>
              <span class="pat-date">{webhook.url}</span>
              <button class="btn btn-ghost btn-sm" onClick={() => handleToggleWebhook(webhook)}>
                {webhook.rowStatus === "NORMAL" ? "停用" : "启用"}
              </button>
              <button class="btn btn-ghost btn-sm" onClick={() => handleDeleteWebhook(webhook)}>
                删除
              </button>
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
              value={webhookName}
              onInput={(e) => setWebhookName((e.target as HTMLInputElement).value)}
            />
          </div>
          <div class="form-group">
            <input
              class="form-input"
              type="url"
              placeholder="https://example.com/webhook"
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

      {currentUser.role === "ADMIN" && (
        <div class="settings-section">
          <h2>数据维护</h2>
          <div class="settings-actions">
            <button class="btn btn-secondary" onClick={handleExport}>
              导出备忘录
            </button>
            <label class="btn btn-secondary file-label">
              导入备忘录
              <input type="file" accept="application/json" onChange={handleImport} />
            </label>
          </div>
        </div>
      )}
    </div>
  );
}
