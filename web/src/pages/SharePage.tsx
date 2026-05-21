import { useState, useEffect } from "preact/hooks";
import { api } from "../api";
import { MarkdownContent } from "../components/MarkdownContent";
import type { Memo } from "../components/MemoCard";

interface SharePageProps {
  path?: string;
  uid?: string;
}

export function SharePage({ uid }: SharePageProps) {
  const [memo, setMemo] = useState<Memo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!uid) return;
    setLoading(true);
    api<{ memo: Memo }>(`/api/v1/shares/${uid}`)
      .then((data) => {
        setMemo(data.memo);
        setError("");
      })
      .catch((err) => {
        setError((err as Error).message);
      })
      .finally(() => setLoading(false));
  }, [uid]);

  const formatDate = (ts: number) => {
    const d = new Date(ts * 1000);
    return d.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  if (loading) {
    return (
      <div class="loading-screen">
        <span class="loading-spinner" />
        加载分享备忘录...
      </div>
    );
  }

  if (error || !memo) {
    return (
      <div class="memo-detail-page">
        <div class="empty-state">
          {error || "分享的备忘录未找到或链接已过期。"}
        </div>
      </div>
    );
  }

  return (
    <div class="memo-detail-page">
      <div class="home-toolbar page-toolbar">
        <div>
          <div class="home-kicker">Shared Memo</div>
          <h1>分享备忘录</h1>
          <p>{formatDate(memo.createdTs)}</p>
        </div>
      </div>

      <div class="memo-card">
        <div class="memo-header">
          <span class="memo-creator">
            {memo.creator.nickname || memo.creator.username}
          </span>
          <span class="memo-time">{formatDate(memo.createdTs)}</span>
          <span class={`memo-visibility vis-${memo.visibility}`}>
            {memo.visibility.toLowerCase()}
          </span>
        </div>

        <MarkdownContent content={memo.content} />

        {memo.attachments && memo.attachments.length > 0 && (
          <div class="memo-attachments">
            {memo.attachments.map((att) => (
              <a
                key={att.uid}
                href={att.url}
                class="memo-attachment"
                target="_blank"
                rel="noopener noreferrer"
              >
                {att.filename}
              </a>
            ))}
          </div>
        )}
      </div>

      <div class="share-footer">
        通过 Memos Worker 分享
      </div>
    </div>
  );
}
