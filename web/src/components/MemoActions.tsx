interface MemoActionsProps {
  isOwner: boolean | null;
  archived: boolean;
  editing: boolean;
  pinned: boolean;
  onEdit: () => void;
  onPin: () => void;
  onArchive: () => void;
  onRestore: () => void;
  onDelete: () => void;
}

export function MemoActions({
  isOwner,
  archived,
  editing,
  pinned,
  onEdit,
  onPin,
  onArchive,
  onRestore,
  onDelete,
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
          <button class="memo-action-icon memo-action-labeled" title="恢复" aria-label="恢复" onClick={onRestore}>
            <span aria-hidden="true">↺</span>
            <span>恢复</span>
          </button>
          <button class="memo-action-icon memo-action-labeled danger" title="彻底删除" aria-label="彻底删除" onClick={onDelete}>
            <span aria-hidden="true">×</span>
            <span>彻底删除</span>
          </button>
        </>
      )}
      {isOwner && !archived && (
        <button class="memo-action-icon memo-action-labeled danger" title="删除" aria-label="删除" onClick={onArchive}>
          <span aria-hidden="true">×</span>
          <span>删除</span>
        </button>
      )}
    </div>
  );
}
