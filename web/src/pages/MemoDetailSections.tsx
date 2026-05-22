import { AttachmentList } from "../components/AttachmentList";
import { MarkdownContent } from "../components/MarkdownContent";
import type { CurrentUser } from "../App";
import type { Memo } from "../components/MemoCard";

export function formatDetailDate(ts: number) {
  const d = new Date(ts * 1000);
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

interface DetailToolbarProps {
  memo: Memo;
  isOwner: boolean;
  isArchived: boolean;
  onArchive: () => void;
  onRestore: () => void;
  onPurge: () => void;
  onBackHome: (event: Event) => void;
}

export function DetailToolbar({
  memo,
  isOwner,
  isArchived,
  onArchive,
  onRestore,
  onPurge,
  onBackHome,
}: DetailToolbarProps) {
  return (
    <div class="home-toolbar page-toolbar">
      <div>
        <div class="home-kicker">Memo</div>
        <h1>备忘录详情</h1>
        <p>{formatDetailDate(memo.createdTs)}</p>
      </div>
      <div class="detail-toolbar-actions">
        {isOwner && (
          isArchived ? (
            <>
              <button class="btn btn-secondary btn-sm" onClick={onRestore}>
                恢复
              </button>
              <button class="btn btn-danger btn-sm" onClick={onPurge}>
                彻底删除
              </button>
            </>
          ) : (
            <button class="btn btn-danger btn-sm" onClick={onArchive}>
              删除
            </button>
          )
        )}
        <a href="/" class="tag-clear" onClick={onBackHome}>
          返回首页
        </a>
      </div>
    </div>
  );
}

interface DetailMemoCardProps {
  memo: Memo;
}

export function DetailMemoCard({
  memo,
}: DetailMemoCardProps) {
  return (
    <div class="memo-card">
      <div class="memo-header">
        <span class="memo-creator">
          {memo.creator.nickname || memo.creator.username}
        </span>
        <span class="memo-time">{formatDetailDate(memo.createdTs)}</span>
        <span class={`memo-visibility vis-${memo.visibility}`}>
          {memo.visibility.toLowerCase()}
        </span>
      </div>

      <MarkdownContent content={memo.content} />
      <AttachmentList attachments={memo.attachments} />
    </div>
  );
}

interface DetailCommentsSectionProps {
  comments: Memo[];
  currentUser: CurrentUser | null;
  commentContent: string;
  commenting: boolean;
  onCommentContentChange: (value: string) => void;
  onAddComment: () => void;
}

export function DetailCommentsSection({
  comments,
  currentUser,
  commentContent,
  commenting,
  onCommentContentChange,
  onAddComment,
}: DetailCommentsSectionProps) {
  return (
    <div class="comments-section">
      <h3>评论 ({comments.length})</h3>

      {comments.map((comment) => (
        <div key={comment.uid} class="memo-card comment-card">
          <div class="memo-header">
            <span class="memo-creator">
              {comment.creator.nickname || comment.creator.username}
            </span>
            <span class="memo-time">{formatDetailDate(comment.createdTs)}</span>
          </div>
          <MarkdownContent content={comment.content} />
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
            onInput={(e) => onCommentContentChange((e.target as HTMLTextAreaElement).value)}
            rows={3}
          />
          <button
            class="btn btn-primary btn-sm"
            onClick={onAddComment}
            disabled={commenting || !commentContent.trim()}
          >
            {commenting ? "发布中..." : "评论"}
          </button>
        </div>
      )}
    </div>
  );
}
