import type { CurrentUser } from "../App";
import { MarkdownContent } from "./MarkdownContent";
import type { Memo, Reaction } from "./MemoCard";

const VISIBILITY_LABEL = { PRIVATE: "私有", PROTECTED: "登录可见", PUBLIC: "公开" };
const VISIBILITY_CLASS = { PRIVATE: "vis-PRIVATE", PROTECTED: "vis-PROTECTED", PUBLIC: "vis-PUBLIC" };

function formatMemoDate(ts: number) {
  const d = new Date(ts * 1000);
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function MemoCardHeader({ memo }: { memo: Memo }) {
  return (
    <div class="memo-header">
      <span class="memo-creator">{memo.creator.nickname || memo.creator.username}</span>
      <span class="memo-dot">·</span>
      <span class="memo-time">{formatMemoDate(memo.createdTs)}</span>
      {memo.visibility !== "PRIVATE" && (
        <span class={`memo-visibility ${VISIBILITY_CLASS[memo.visibility]}`}>
          {VISIBILITY_LABEL[memo.visibility]}
        </span>
      )}
      {memo.pinned && <span style={{ fontSize: "0.75rem", color: "var(--zinc-400)" }}>📌</span>}
    </div>
  );
}

interface MemoCardEditorProps {
  content: string;
  visibility: Memo["visibility"];
  saving: boolean;
  onContentChange: (value: string) => void;
  onVisibilityChange: (value: Memo["visibility"]) => void;
  onCancel: () => void;
  onSave: () => void;
}

export function MemoCardEditor({
  content,
  visibility,
  saving,
  onContentChange,
  onVisibilityChange,
  onCancel,
  onSave,
}: MemoCardEditorProps) {
  return (
    <div>
      <textarea
        class="editor-textarea"
        value={content}
        onInput={(e) => onContentChange((e.target as HTMLTextAreaElement).value)}
        style={{ minHeight: "80px", border: "1px solid var(--zinc-200)", borderRadius: "6px", padding: "10px 12px" }}
      />
      <div class="editor-actions">
        <select value={visibility} onChange={(e) => onVisibilityChange((e.target as HTMLSelectElement).value as Memo["visibility"])}>
          <option value="PRIVATE">私有</option>
          <option value="PROTECTED">登录可见</option>
          <option value="PUBLIC">公开</option>
        </select>
        <div class="spacer" />
        <button class="btn btn-ghost btn-sm" onClick={onCancel}>取消</button>
        <button class="btn btn-primary btn-sm" onClick={onSave} disabled={saving}>
          {saving ? "保存中..." : "保存"}
        </button>
      </div>
    </div>
  );
}

interface ReactionListProps {
  reactions: Reaction[];
  currentUser: CurrentUser | null;
  onRemove: (reactionId: number) => void;
}

export function ReactionList({ reactions, currentUser, onRemove }: ReactionListProps) {
  if (reactions.length === 0) return null;
  return (
    <div class="memo-reactions">
      {reactions.map((reaction) => {
        const isMine = currentUser && reaction.creator.id === currentUser.id;
        return (
          <button
            key={reaction.id}
            class={`reaction-chip${isMine ? " mine" : ""}`}
            onClick={() => isMine && onRemove(reaction.id)}
            title={reaction.creator.username}
          >
            {reaction.reactionType}
          </button>
        );
      })}
    </div>
  );
}

export function ReactionPicker({
  options,
  onAdd,
}: {
  options: string[];
  onAdd: (reactionType: string) => void;
}) {
  return (
    <div class="memo-reactions" style={{ marginTop: "6px" }}>
      {options.map((emoji) => (
        <button
          key={emoji}
          class="reaction-chip"
          onClick={() => onAdd(emoji)}
        >
          {emoji}
        </button>
      ))}
    </div>
  );
}

interface CommentsSectionProps {
  comments: Memo[];
  commentsLoaded: boolean;
  currentUser: CurrentUser | null;
  commentContent: string;
  commenting: boolean;
  onCommentContentChange: (value: string) => void;
  onAddComment: () => void;
}

export function CommentsSection({
  comments,
  commentsLoaded,
  currentUser,
  commentContent,
  commenting,
  onCommentContentChange,
  onAddComment,
}: CommentsSectionProps) {
  return (
    <div class="comments-section" style={{ marginTop: "12px", paddingTop: "12px", borderTop: "1px solid var(--zinc-100)" }}>
      {comments.length > 0 ? (
        comments.map((comment) => (
          <div key={comment.uid} style={{ padding: "10px 0", borderBottom: "1px solid var(--zinc-50)" }}>
            <div class="memo-header" style={{ marginBottom: 4 }}>
              <span class="memo-creator" style={{ fontSize: "0.8125rem" }}>
                {comment.creator.nickname || comment.creator.username}
              </span>
              <span class="memo-dot">·</span>
              <span class="memo-time">{formatMemoDate(comment.createdTs)}</span>
            </div>
            <MarkdownContent content={comment.content} />
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
            onInput={(e) => onCommentContentChange((e.target as HTMLTextAreaElement).value)}
            style={{ minHeight: "56px" }}
          />
          <button class="btn btn-primary btn-sm" onClick={onAddComment} disabled={commenting || !commentContent.trim()}>
            {commenting ? "发布中..." : "评论"}
          </button>
        </div>
      )}
    </div>
  );
}

export function ShareUrlBox({
  shareUrl,
  onCopy,
}: {
  shareUrl: string;
  onCopy: () => void;
}) {
  return (
    <div class="share-url-box">
      <input type="text" readOnly value={shareUrl} onClick={(e) => (e.target as HTMLInputElement).select()} />
      <button class="btn btn-ghost btn-sm" onClick={onCopy}>
        复制
      </button>
    </div>
  );
}
