import { useCallback, useEffect, useState } from "preact/hooks";
import { api } from "../api";
import { buildShareUrl } from "../integrationHelpers";
import { useFeedback } from "./Feedback";

interface Share {
  id: number;
  uid: string;
  createdTs: number;
  expiresTs: number | null;
}

interface ShareManagerProps {
  memoUid: string;
}

export function ShareManager({ memoUid }: ShareManagerProps) {
  const { notify, confirm } = useFeedback();
  const [shares, setShares] = useState<Share[]>([]);
  const [expiresAt, setExpiresAt] = useState("");
  const [creating, setCreating] = useState(false);

  const fetchShares = useCallback(async () => {
    const data = await api<{ shares: Share[] }>(`/api/v1/memos/${memoUid}/shares`);
    setShares(data.shares);
  }, [memoUid]);

  useEffect(() => {
    fetchShares().catch(() => undefined);
  }, [fetchShares]);

  const createShare = async () => {
    setCreating(true);
    try {
      const expiresTs = expiresAt ? Math.floor(new Date(expiresAt).getTime() / 1000) : undefined;
      const data = await api<{ share: Share }>(`/api/v1/memos/${memoUid}/shares`, {
        method: "POST",
        body: JSON.stringify({ expiresTs }),
      });
      const url = buildShareUrl(window.location.origin, data.share.uid);
      await navigator.clipboard.writeText(url).catch(() => undefined);
      notify("分享链接已创建并复制", "success");
      setExpiresAt("");
      fetchShares();
    } catch (err) {
      notify(`创建分享失败：${(err as Error).message}`, "error");
    } finally {
      setCreating(false);
    }
  };

  const deleteShare = async (share: Share) => {
    const ok = await confirm({
      title: "删除这个分享链接？",
      message: "删除后该公开链接会立即失效。",
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    await api(`/api/v1/memos/${memoUid}/shares/${share.id}`, { method: "DELETE" });
    notify("分享链接已删除", "success");
    fetchShares();
  };

  const formatTs = (ts: number) => new Date(ts * 1000).toLocaleString();

  return (
    <div class="settings-section compact-section">
      <h2>分享管理</h2>
      <div class="inline-form">
        <div class="form-group">
          <label class="form-label">过期时间</label>
          <input
            class="form-input"
            type="datetime-local"
            value={expiresAt}
            onInput={(e) => setExpiresAt((e.target as HTMLInputElement).value)}
          />
        </div>
        <button class="btn btn-primary btn-sm" onClick={createShare} disabled={creating}>
          {creating ? "创建中..." : "创建并复制"}
        </button>
      </div>

      <div class="settings-record-list section-list">
        {shares.map((share) => {
          const url = buildShareUrl(window.location.origin, share.uid);
          return (
            <div key={share.id} class="settings-record-row">
              <div class="settings-record-main">
                <span class="settings-record-title">{share.uid}</span>
                <span class="settings-record-meta">{share.expiresTs ? `过期 ${formatTs(share.expiresTs)}` : "永久有效"}</span>
              </div>
              <div class="settings-record-actions">
                <button class="btn btn-ghost btn-sm" onClick={() => navigator.clipboard.writeText(url)}>
                  复制
                </button>
                <button class="btn btn-danger-soft btn-sm" onClick={() => deleteShare(share)}>
                  删除
                </button>
              </div>
            </div>
          );
        })}
        {shares.length === 0 && <div class="muted-line">暂无分享链接。</div>}
      </div>
    </div>
  );
}
