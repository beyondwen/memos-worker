interface MemoActionsProps {
  isOwner: boolean | null;
  archived: boolean;
  editing: boolean;
  commentCount?: number;
  onOpen: () => void;
  onEdit: () => void;
  onArchive: () => void;
  onRestore: () => void;
  onReact: () => void;
  onComments: () => void;
  onShare: () => void;
}

export function MemoActions({
  isOwner,
  archived,
  editing,
  commentCount = 0,
  onOpen,
  onEdit,
  onArchive,
  onRestore,
  onReact,
  onComments,
  onShare,
}: MemoActionsProps) {
  return (
    <div class="memo-actions">
      <button class="memo-action-icon" title="查看详情" aria-label="查看详情" onClick={onOpen}>
        <span aria-hidden="true">↗</span>
      </button>
      {isOwner && !editing && (
        <button class="memo-action-icon" title="编辑" aria-label="编辑" onClick={onEdit}>
          <span aria-hidden="true">✎</span>
        </button>
      )}
      {isOwner && archived && (
        <button class="memo-action-icon" title="恢复" aria-label="恢复" onClick={onRestore}>
          <span aria-hidden="true">↺</span>
        </button>
      )}
      {isOwner && !archived && (
        <button class="memo-action-icon" title="归档" aria-label="归档" onClick={onArchive}>
          <span aria-hidden="true">▣</span>
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
