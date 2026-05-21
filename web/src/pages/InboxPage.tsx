import { useCallback, useEffect, useState } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { formatInboxItem, type InboxItem } from "../inboxView";
import { useFeedback } from "../components/Feedback";
import type { CurrentUser } from "../App";

interface InboxPageProps {
  path: string;
  currentUser: CurrentUser | null;
}

export function InboxPage({ currentUser }: InboxPageProps) {
  const { notify } = useFeedback();
  const [items, setItems] = useState<InboxItem[]>([]);
  const [unreadCount, setUnreadCount] = useState(0);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!currentUser) route("/auth", true);
  }, [currentUser]);

  const fetchInbox = useCallback(async () => {
    if (!currentUser) return;
    setLoading(true);
    try {
      const data = await api<{ inbox: InboxItem[]; unreadCount: number }>("/api/v1/inbox");
      setItems(data.inbox);
      setUnreadCount(data.unreadCount);
    } catch (err) {
      notify(`加载通知失败：${(err as Error).message}`, "error");
    } finally {
      setLoading(false);
    }
  }, [currentUser, notify]);

  useEffect(() => {
    fetchInbox();
  }, [fetchInbox]);

  if (!currentUser) return null;

  const markRead = async (ids?: number[]) => {
    await api("/api/v1/inbox", {
      method: "PATCH",
      body: JSON.stringify({ ids, status: "READ" }),
    });
    fetchInbox();
    window.dispatchEvent(new CustomEvent("memos:inbox-refresh"));
  };

  const removeItem = async (id: number) => {
    await api(`/api/v1/inbox/${id}`, { method: "DELETE" });
    fetchInbox();
    window.dispatchEvent(new CustomEvent("memos:inbox-refresh"));
  };

  return (
    <div class="settings-layout">
      <div class="home-toolbar page-toolbar">
        <div>
          <div class="home-kicker">Inbox</div>
          <h1>通知</h1>
          <p>{unreadCount > 0 ? `${unreadCount} 条未读` : "没有未读通知"}</p>
        </div>
        {items.length > 0 && (
          <button class="tag-clear" onClick={() => markRead()}>
            全部已读
          </button>
        )}
      </div>

      <div class="settings-section">
        {loading && (
          <div class="loading-screen loading-inline">
            <span class="loading-spinner" />
          </div>
        )}

        {!loading && items.length === 0 && (
          <div class="muted-line">暂无通知。</div>
        )}

        <div class="inbox-list">
          {items.map((item) => {
            const display = formatInboxItem(item);
            return (
              <div key={item.id} class={`inbox-item ${item.status === "UNREAD" ? "unread" : ""}`}>
                <button
                  class="inbox-main"
                  onClick={() => {
                    markRead([item.id]);
                    if (display.memoPath) route(display.memoPath);
                  }}
                >
                  <span class="inbox-title">{display.title}</span>
                  <span class="inbox-detail">{display.detail}</span>
                </button>
                <div class="inbox-actions">
                  {item.status === "UNREAD" && (
                    <button class="btn btn-ghost btn-sm" onClick={() => markRead([item.id])}>
                      已读
                    </button>
                  )}
                  <button class="btn btn-ghost btn-sm" onClick={() => removeItem(item.id)}>
                    删除
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
