import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api, getToken } from "../api";
import { MarkdownContent } from "../components/MarkdownContent";
import { useFeedback } from "../components/Feedback";
import { AttachmentList } from "../components/AttachmentList";
import { ShareManager } from "../components/ShareManager";
import { createMemoEventSource, shouldRefreshForSseEvent } from "../sseEvents";
import { buildShareUrl } from "../integrationHelpers";
import type { CurrentUser } from "../App";
import type { Memo, Reaction } from "../components/MemoCard";

interface MemoDetailPageProps {
  path?: string;
  uid?: string;
  currentUser: CurrentUser | null;
}

const EMOJI_OPTIONS = ["👍", "❤️", "😄", "🎉", "🤔", "👀"];

export function MemoDetailPage({ uid, currentUser }: MemoDetailPageProps) {
  const { notify } = useFeedback();
  const [memo, setMemo] = useState<Memo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [comments, setComments] = useState<Memo[]>([]);
  const [commentContent, setCommentContent] = useState("");
  const [commenting, setCommenting] = useState(false);
  const [reactions, setReactions] = useState<Reaction[]>([]);
  const [showReactionPicker, setShowReactionPicker] = useState(false);
  const [shareUrl, setShareUrl] = useState("");
  const [showShare, setShowShare] = useState(false);

  const fetchMemo = useCallback(async () => {
    if (!uid) return;
    setLoading(true);
    try {
      const data = await api<{ memo: Memo }>(`/api/v1/memos/${uid}`);
      setMemo(data.memo);
      setError("");
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  }, [uid]);

  const fetchComments = useCallback(async () => {
    if (!uid) return;
    try {
      const data = await api<{ memos: Memo[] }>(
        `/api/v1/memos/${uid}/comments`
      );
      setComments(data.memos);
    } catch {
      // ignore
    }
  }, [uid]);

  const fetchReactions = useCallback(async () => {
    if (!uid) return;
    try {
      const data = await api<{ reactions: Reaction[] }>(
        `/api/v1/memos/${uid}/reactions`
      );
      setReactions(data.reactions);
    } catch {
      // ignore
    }
  }, [uid]);

  useEffect(() => {
    fetchMemo();
    fetchComments();
    fetchReactions();
  }, [fetchMemo, fetchComments, fetchReactions]);

  useEffect(() => {
    if (!currentUser || !uid) return;
    const source = createMemoEventSource(getToken());
    if (!source) return;
    const refresh = (message: MessageEvent) => {
      try {
        const event = JSON.parse(message.data);
        if (!shouldRefreshForSseEvent(event) || event.name !== `memos/${uid}`) return;
        fetchMemo();
        fetchComments();
        fetchReactions();
      } catch {
        // Ignore malformed SSE payloads.
      }
    };
    for (const type of ["memo.updated", "memo.deleted", "memo.comment.created", "reaction.upserted", "reaction.deleted"]) {
      source.addEventListener(type, refresh);
    }
    return () => source.close();
  }, [currentUser, fetchComments, fetchMemo, fetchReactions, uid]);

  const handleAddComment = async () => {
    if (!uid || !commentContent.trim()) return;
    setCommenting(true);
    try {
      const data = await api<{ memo: Memo }>(
        `/api/v1/memos/${uid}/comments`,
        {
          method: "POST",
          body: JSON.stringify({ content: commentContent.trim() }),
        }
      );
      if (data.memo) setComments((prev) => [...prev, data.memo]);
      setCommentContent("");
    } catch (err) {
      notify(`评论失败：${(err as Error).message}`, "error");
    } finally {
      setCommenting(false);
    }
  };

  const addReaction = async (reactionType: string) => {
    if (!uid) return;
    setShowReactionPicker(false);
    try {
      const data = await api<{ reactions: Reaction[] }>(
        `/api/v1/memos/${uid}/reactions`,
        {
          method: "POST",
          body: JSON.stringify({ reactionType }),
        }
      );
      setReactions(data.reactions);
    } catch (err) {
      notify(`表态失败：${(err as Error).message}`, "error");
    }
  };

  const removeReaction = async (reactionId: number) => {
    if (!uid) return;
    try {
      const data = await api<{ reactions: Reaction[] }>(
        `/api/v1/memos/${uid}/reactions/${reactionId}`,
        { method: "DELETE" }
      );
      setReactions(data.reactions);
    } catch (err) {
      notify(`取消表态失败：${(err as Error).message}`, "error");
    }
  };

  const handleShare = async () => {
    if (!uid) return;
    if (showShare) {
      setShowShare(false);
      return;
    }
    try {
      const data = await api<{ share: { uid: string } }>(
        `/api/v1/memos/${uid}/shares`,
        { method: "POST", body: JSON.stringify({}) }
      );
      setShareUrl(buildShareUrl(window.location.origin, data.share.uid));
      setShowShare(true);
    } catch (err) {
      notify(`分享失败：${(err as Error).message}`, "error");
    }
  };

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
      </div>
    );
  }

  if (error || !memo) {
    return (
      <div class="memo-detail-page">
        <a
          href="/"
          class="back-link"
          onClick={(e) => { e.preventDefault(); route("/"); }}
        >
          &larr; 返回
        </a>
        <div class="empty-state">{error || "备忘录未找到"}</div>
      </div>
    );
  }

  return (
    <div class="memo-detail-page">
      <div class="home-toolbar page-toolbar">
        <div>
          <div class="home-kicker">Memo</div>
          <h1>备忘录详情</h1>
          <p>{formatDate(memo.createdTs)}</p>
        </div>
        <a
          href="/"
          class="tag-clear"
          onClick={(e) => { e.preventDefault(); route("/"); }}
        >
          返回首页
        </a>
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

        <AttachmentList attachments={memo.attachments} />

        {reactions.length > 0 && (
          <div class="memo-reactions">
            {reactions.map((r) => {
              const isMine = currentUser && r.creator.id === currentUser.id;
              return (
                <button
                  key={r.id}
                  class={`reaction-chip${isMine ? " mine" : ""}`}
                  onClick={() => isMine && removeReaction(r.id)}
                  title={r.creator.username}
                >
                  {r.reactionType}
                </button>
              );
            })}
          </div>
        )}

        {showReactionPicker && (
          <div class="memo-reactions reaction-picker">
            {EMOJI_OPTIONS.map((emoji) => (
              <button
                key={emoji}
                class="reaction-chip"
                onClick={() => addReaction(emoji)}
              >
                {emoji}
              </button>
            ))}
          </div>
        )}

        {showShare && shareUrl && (
          <div class="share-url-box">
            <input type="text" readOnly value={shareUrl} onClick={(e) => (e.target as HTMLInputElement).select()} />
            <button
              class="btn btn-ghost btn-sm"
              onClick={() => {
                navigator.clipboard.writeText(shareUrl);
                notify("分享链接已复制", "success");
              }}
            >
              复制
            </button>
          </div>
        )}

        <div class="memo-actions">
          <button onClick={() => setShowReactionPicker(!showReactionPicker)}>
            表态
          </button>
          <button onClick={handleShare}>
            分享
          </button>
        </div>
      </div>

      {currentUser?.id === memo.creator.id && (
        <ShareManager memoUid={memo.uid} />
      )}

      <div class="comments-section">
        <h3>评论 ({comments.length})</h3>

        {comments.map((c) => (
          <div key={c.uid} class="memo-card comment-card">
            <div class="memo-header">
              <span class="memo-creator">
                {c.creator.nickname || c.creator.username}
              </span>
              <span class="memo-time">{formatDate(c.createdTs)}</span>
            </div>
            <MarkdownContent content={c.content} />
          </div>
        ))}

        {comments.length === 0 && (
          <div class="muted-line">
            暂无评论。
          </div>
        )}

        {currentUser && (
          <div class="comment-form">
            <textarea
              class="editor-textarea"
              placeholder="写评论..."
              value={commentContent}
              onInput={(e) =>
                setCommentContent((e.target as HTMLTextAreaElement).value)
              }
              rows={3}
            />
            <button
              class="btn btn-primary btn-sm"
              onClick={handleAddComment}
              disabled={commenting || !commentContent.trim()}
            >
              {commenting ? "发布中..." : "评论"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
