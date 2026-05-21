import { useState, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { MarkdownContent } from "./MarkdownContent";
import { useFeedback } from "./Feedback";
import { AttachmentList } from "./AttachmentList";
import type { CurrentUser } from "../App";

export interface Memo {
  name: string;
  id: number;
  uid: string;
  creator: { id: number; username: string; nickname: string };
  createdTs: number;
  updatedTs: number;
  rowStatus: string;
  content: string;
  visibility: "PUBLIC" | "PROTECTED" | "PRIVATE";
  pinned: boolean;
  payload: { tags?: string[] };
  attachments?: Attachment[];
}

export interface Attachment {
  name: string;
  uid: string;
  filename: string;
  type: string;
  size: number;
  memoId: number | null;
  createdTs: number;
  url: string;
}

export interface Reaction {
  id: number;
  reactionType: string;
  creator: { id: number; username: string };
  createdTs: number;
}

interface MemoCardProps {
  memo: Memo;
  currentUser: CurrentUser | null;
  onUpdate?: (memo: Memo) => void;
}

const EMOJI_OPTIONS = ["👍", "❤️", "😄", "🎉", "🤔", "👀"];

export function MemoCard({ memo, currentUser, onUpdate }: MemoCardProps) {
  const { notify, confirm } = useFeedback();
  const [editing, setEditing] = useState(false);
  const [editContent, setEditContent] = useState(memo.content);
  const [editVisibility, setEditVisibility] = useState(memo.visibility);
  const [saving, setSaving] = useState(false);
  const [reactions, setReactions] = useState<Reaction[]>([]);
  const [reactionsLoaded, setReactionsLoaded] = useState(false);
  const [showReactionPicker, setShowReactionPicker] = useState(false);
  const [showComments, setShowComments] = useState(false);
  const [comments, setComments] = useState<Memo[]>([]);
  const [commentsLoaded, setCommentsLoaded] = useState(false);
  const [commentContent, setCommentContent] = useState("");
  const [commenting, setCommenting] = useState(false);
  const [shareUrl, setShareUrl] = useState("");
  const [showShare, setShowShare] = useState(false);

  const isOwner = currentUser && memo.creator.id === currentUser.id;

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

  const handleSave = async () => {
    const trimmed = editContent.trim();
    if (!trimmed) return;
    setSaving(true);
    try {
      const data = await api<{ memo: Memo }>(`/api/v1/memos/${memo.uid}`, {
        method: "PATCH",
        body: JSON.stringify({ content: trimmed, visibility: editVisibility }),
      });
      setEditing(false);
      onUpdate?.(data.memo);
      notify("备忘录已保存", "success");
    } catch (err) {
      notify(`保存失败：${(err as Error).message}`, "error");
    } finally {
      setSaving(false);
    }
  };

  const handleArchive = async () => {
    const ok = await confirm({
      title: "归档这条备忘录？",
      message: "归档后会从当前列表中移除。",
      confirmText: "归档",
      danger: true,
    });
    if (!ok) return;
    try {
      await api(`/api/v1/memos/${memo.uid}`, {
        method: "PATCH",
        body: JSON.stringify({ rowStatus: "ARCHIVED" }),
      });
      onUpdate?.({ ...memo, rowStatus: "ARCHIVED" });
      notify("备忘录已归档", "success");
    } catch (err) {
      notify(`归档失败：${(err as Error).message}`, "error");
    }
  };

  const handleRestore = async () => {
    try {
      const data = await api<{ memo: Memo }>(`/api/v1/memos/${memo.uid}`, {
        method: "PATCH",
        body: JSON.stringify({ rowStatus: "NORMAL" }),
      });
      onUpdate?.(data.memo);
      notify("备忘录已恢复", "success");
    } catch (err) {
      notify(`恢复失败：${(err as Error).message}`, "error");
    }
  };

  const loadReactions = useCallback(async () => {
    if (reactionsLoaded) return;
    try {
      const data = await api<{ reactions: Reaction[] }>(
        `/api/v1/memos/${memo.uid}/reactions`
      );
      setReactions(data.reactions);
      setReactionsLoaded(true);
    } catch {
      // ignore
    }
  }, [memo.uid, reactionsLoaded]);

  const addReaction = async (reactionType: string) => {
    setShowReactionPicker(false);
    try {
      const data = await api<{ reactions: Reaction[] }>(
        `/api/v1/memos/${memo.uid}/reactions`,
        {
          method: "POST",
          body: JSON.stringify({ reactionType }),
        }
      );
      setReactions(data.reactions);
      setReactionsLoaded(true);
    } catch (err) {
      notify(`表态失败：${(err as Error).message}`, "error");
    }
  };

  const removeReaction = async (reactionId: number) => {
    try {
      const data = await api<{ reactions: Reaction[] }>(
        `/api/v1/memos/${memo.uid}/reactions/${reactionId}`,
        { method: "DELETE" }
      );
      setReactions(data.reactions);
    } catch (err) {
      notify(`取消表态失败：${(err as Error).message}`, "error");
    }
  };

  const loadComments = useCallback(async () => {
    if (commentsLoaded) return;
    try {
      const data = await api<{ memos: Memo[] }>(
        `/api/v1/memos/${memo.uid}/comments`
      );
      setComments(data.memos);
      setCommentsLoaded(true);
    } catch {
      // ignore
    }
  }, [memo.uid, commentsLoaded]);

  const handleToggleComments = () => {
    const next = !showComments;
    setShowComments(next);
    if (next) loadComments();
  };

  const handleAddComment = async () => {
    const trimmed = commentContent.trim();
    if (!trimmed) return;
    setCommenting(true);
    try {
      const data = await api<{ memo: Memo }>(
        `/api/v1/memos/${memo.uid}/comments`,
        {
          method: "POST",
          body: JSON.stringify({ content: trimmed }),
        }
      );
      if (data.memo) {
        setComments((prev) => [...prev, data.memo]);
      }
      setCommentContent("");
    } catch (err) {
      notify(`评论失败：${(err as Error).message}`, "error");
    } finally {
      setCommenting(false);
    }
  };

  const handleShare = async () => {
    if (showShare) {
      setShowShare(false);
      return;
    }
    try {
      const data = await api<{ share: { uid: string; url: string } }>(
        `/api/v1/memos/${memo.uid}/shares`,
        { method: "POST", body: JSON.stringify({}) }
      );
      const full = `${window.location.origin}/#/shares/${data.share.uid}`;
      setShareUrl(full);
      setShowShare(true);
    } catch (err) {
      notify(`分享失败：${(err as Error).message}`, "error");
    }
  };

  const handleToggleReactions = () => {
    const next = !showReactionPicker;
    if (next) loadReactions();
    setShowReactionPicker(next);
  };

  const visLabel = { PRIVATE: "私有", PROTECTED: "登录可见", PUBLIC: "公开" };
  const visClass = { PRIVATE: "vis-PRIVATE", PROTECTED: "vis-PROTECTED", PUBLIC: "vis-PUBLIC" };

  return (
    <div class="memo-card">
      <div class="memo-header">
        <span class="memo-creator">{memo.creator.nickname || memo.creator.username}</span>
        <span class="memo-dot">·</span>
        <span class="memo-time">{formatDate(memo.createdTs)}</span>
        {memo.visibility !== "PRIVATE" && (
          <span class={`memo-visibility ${visClass[memo.visibility]}`}>
            {visLabel[memo.visibility]}
          </span>
        )}
        {memo.pinned && <span style={{ fontSize: "0.75rem", color: "var(--zinc-400)" }}>📌</span>}
      </div>

      {editing ? (
        <div>
          <textarea
            class="editor-textarea"
            value={editContent}
            onInput={(e) => setEditContent((e.target as HTMLTextAreaElement).value)}
            style={{ minHeight: "80px", border: "1px solid var(--zinc-200)", borderRadius: "6px", padding: "10px 12px" }}
          />
          <div class="editor-actions">
            <select value={editVisibility} onChange={(e) => setEditVisibility((e.target as HTMLSelectElement).value as "PRIVATE" | "PROTECTED" | "PUBLIC")}>
              <option value="PRIVATE">私有</option>
              <option value="PROTECTED">登录可见</option>
              <option value="PUBLIC">公开</option>
            </select>
            <div class="spacer" />
            <button class="btn btn-ghost btn-sm" onClick={() => setEditing(false)}>取消</button>
            <button class="btn btn-primary btn-sm" onClick={handleSave} disabled={saving}>
              {saving ? "保存中..." : "保存"}
            </button>
          </div>
        </div>
      ) : (
        <MarkdownContent content={memo.content} />
      )}

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
        <div class="memo-reactions" style={{ marginTop: "6px" }}>
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

      {showComments && (
        <div class="comments-section" style={{ marginTop: "12px", paddingTop: "12px", borderTop: "1px solid var(--zinc-100)" }}>
          {comments.length > 0 ? (
            comments.map((c) => (
              <div key={c.uid} style={{ padding: "10px 0", borderBottom: "1px solid var(--zinc-50)" }}>
                <div class="memo-header" style={{ marginBottom: 4 }}>
                  <span class="memo-creator" style={{ fontSize: "0.8125rem" }}>
                    {c.creator.nickname || c.creator.username}
                  </span>
                  <span class="memo-dot">·</span>
                  <span class="memo-time">{formatDate(c.createdTs)}</span>
                </div>
                <MarkdownContent content={c.content} />
              </div>
            ))
          ) : (
            commentsLoaded && (
              <div style={{ color: "var(--zinc-300)", fontSize: "0.8125rem", padding: "8px 0" }}>
                暂无评论
              </div>
            )
          )}

          {currentUser && (
            <div class="comment-form">
              <textarea
                class="editor-textarea"
                placeholder="写评论..."
                value={commentContent}
                onInput={(e) => setCommentContent((e.target as HTMLTextAreaElement).value)}
                style={{ minHeight: "56px" }}
              />
              <button class="btn btn-primary btn-sm" onClick={handleAddComment} disabled={commenting || !commentContent.trim()}>
                {commenting ? "发布中..." : "评论"}
              </button>
            </div>
          )}
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
        <button class="memo-action-icon" title="查看详情" aria-label="查看详情" onClick={() => route(`/memos/${memo.uid}`)}>
          ↗
        </button>
        {isOwner && !editing && (
          <button class="memo-action-icon" title="编辑" aria-label="编辑" onClick={() => { setEditContent(memo.content); setEditVisibility(memo.visibility); setEditing(true); }}>
            ✎
          </button>
        )}
        {isOwner && memo.rowStatus === "ARCHIVED" && (
          <button class="memo-action-icon" title="恢复" aria-label="恢复" onClick={handleRestore}>
            ↺
          </button>
        )}
        {isOwner && memo.rowStatus !== "ARCHIVED" && (
          <button class="memo-action-icon" title="归档" aria-label="归档" onClick={handleArchive}>
            □
          </button>
        )}
        <button class="memo-action-icon" title="表态" aria-label="表态" onClick={handleToggleReactions}>+</button>
        <button class="memo-action-icon" title="评论" aria-label="评论" onClick={handleToggleComments}>
          ◌{commentsLoaded && comments.length > 0 ? comments.length : ""}
        </button>
        <button class="memo-action-icon" title="分享" aria-label="分享" onClick={handleShare}>⌁</button>
      </div>
    </div>
  );
}
