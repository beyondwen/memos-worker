interface MemoActionsProps {
  isOwner: boolean | null;
  archived: boolean;
  editing: boolean;
  pinned: boolean;
  commentCount?: number;
  onEdit: () => void;
  onPin: () => void;
  onArchive: () => void;
  onRestore: () => void;
  onDelete: () => void;
  onReact: () => void;
  onComments: () => void;
  onShare: () => void;
}

export function MemoActions({
  isOwner,
  archived,
  editing,
  pinned,
  commentCount = 0,
  onEdit,
  onPin,
  onArchive,
  onRestore,
  onDelete,
  onReact,
  onComments,
  onShare,
}: MemoActionsProps) {
  return (
    <div class="memo-actions">
      {isOwner && !editing && (
        <button class="memo-action-icon" title="编辑" aria-label="编辑" onClick={onEdit}>
          <span aria-hidden="true">✎</span>
        </button>
      )}
      {isOwner && !editing && !archived && (
        <button class="memo-action-icon" title={pinned ? "取消置顶" : "置顶"} aria-label={pinned ? "取消置顶" : "置顶"} onClick={onPin}>
          <span aria-hidden="true">{pinned ? "★" : "☆"}</span>
        </button>
      )}
      {isOwner && archived && (
        <>
          <button class="memo-action-icon" title="恢复" aria-label="恢复" onClick={onRestore}>
            <span aria-hidden="true">↺</span>
          </button>
          <button class="memo-action-icon danger" title="彻底删除" aria-label="彻底删除" onClick={onDelete}>
            <span aria-hidden="true">×</span>
          </button>
        </>
      )}
      {isOwner && !archived && (
        <button class="memo-action-icon danger" title="删除" aria-label="删除" onClick={onArchive}>
          <span aria-hidden="true">×</span>
        </button>
      )}
      <button class="memo-action-icon" title="表态" aria-label="表态" onClick={onReact}>
        <span aria-hidden="true">♡</span>
      </button>
      <button class="memo-action-icon" title="评论" aria-label="评论" onClick={onComments}>
        <span aria-hidden="true">☰</span>{commentCount > 0 ? commentCount : ""}
      </button>
      <button class="memo-action-icon" title="分享" aria-label="分享" onClick={onShare}>
        <span aria-hidden="true">⛓</span>
      </button>
    </div>
  );
}
