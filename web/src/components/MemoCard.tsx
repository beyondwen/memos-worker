import { useState } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { MarkdownContent } from "./MarkdownContent";
import { useFeedback } from "./Feedback";
import { AttachmentList } from "./AttachmentList";
import { MemoActions } from "./MemoActions";
import { shouldOpenMemoDetailFromCardClick } from "../cardClick";
import type { CurrentUser } from "../App";
import { MemoCardEditor, MemoCardHeader } from "./MemoCardSections";

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

interface MemoCardProps {
  memo: Memo;
  currentUser: CurrentUser | null;
  onUpdate?: (memo: Memo) => void;
  onRemove?: (uid: string) => void;
  selectionMode?: boolean;
  selected?: boolean;
  onSelect?: (uid: string, checked: boolean) => void;
  highlight?: string;
}

export function MemoCard({
  memo,
  currentUser,
  onUpdate,
  onRemove,
  selectionMode = false,
  selected = false,
  onSelect,
  highlight = "",
}: MemoCardProps) {
  const { notify, confirm } = useFeedback();
  const [editing, setEditing] = useState(false);
  const [editContent, setEditContent] = useState(memo.content);
  const [editVisibility, setEditVisibility] = useState(memo.visibility);
  const [saving, setSaving] = useState(false);

  const isOwner = currentUser && memo.creator.id === currentUser.id;

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
      title: "删除这条备忘录？",
      message: "会移入回收站，之后可以恢复或彻底删除。",
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await api(`/api/v1/memos/${memo.uid}`, {
        method: "PATCH",
        body: JSON.stringify({ rowStatus: "ARCHIVED" }),
      });
      onUpdate?.({ ...memo, rowStatus: "ARCHIVED" });
      notify("备忘录已删除到回收站", "success");
    } catch (err) {
      notify(`删除失败：${(err as Error).message}`, "error");
    }
  };

  const handleTogglePinned = async () => {
    try {
      const data = await api<{ memo: Memo }>(`/api/v1/memos/${memo.uid}`, {
        method: "PATCH",
        body: JSON.stringify({ pinned: !memo.pinned }),
      });
      onUpdate?.(data.memo);
      notify(data.memo.pinned ? "备忘录已置顶" : "已取消置顶", "success");
    } catch (err) {
      notify(`更新置顶失败：${(err as Error).message}`, "error");
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

  const handlePurge = async () => {
    const ok = await confirm({
      title: "彻底删除这条备忘录？",
      message: "这会永久移除备忘录，附件会解绑但不会一并删除。",
      confirmText: "彻底删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await api(`/api/v1/memos/${memo.uid}?purge=true`, { method: "DELETE" });
      onRemove?.(memo.uid);
      notify("备忘录已彻底删除", "success");
    } catch (err) {
      notify(`彻底删除失败：${(err as Error).message}`, "error");
    }
  };

  const openDetail = () => route(`/memos/${memo.uid}`);

  const handleCardClick = (event: MouseEvent) => {
    if (selectionMode && isOwner && shouldOpenMemoDetailFromCardClick(event.target, editing)) {
      onSelect?.(memo.uid, !selected);
      return;
    }
    if (shouldOpenMemoDetailFromCardClick(event.target, editing)) openDetail();
  };

  const handleCardKeyDown = (event: KeyboardEvent) => {
    if (editing || event.defaultPrevented) return;
    if (event.key !== "Enter" && event.key !== " ") return;
    if (!shouldOpenMemoDetailFromCardClick(event.target, false)) return;
    event.preventDefault();
    if (selectionMode && isOwner) {
      onSelect?.(memo.uid, !selected);
      return;
    }
    openDetail();
  };

  return (
    <div
      class={`memo-card${editing ? "" : " clickable"}`}
      role={editing ? undefined : selectionMode && isOwner ? "button" : "link"}
      aria-pressed={selectionMode && isOwner ? selected : undefined}
      tabIndex={editing ? undefined : 0}
      onClick={handleCardClick}
      onKeyDown={handleCardKeyDown}
    >
      <MemoCardHeader memo={memo} />

      {editing ? (
        <MemoCardEditor
          content={editContent}
          visibility={editVisibility}
          saving={saving}
          onContentChange={setEditContent}
          onVisibilityChange={setEditVisibility}
          onCancel={() => setEditing(false)}
          onSave={handleSave}
        />
      ) : (
        <MarkdownContent content={memo.content} highlight={highlight} />
      )}

      <AttachmentList attachments={memo.attachments} />

      {!selectionMode && (
        <MemoActions
          isOwner={isOwner}
          archived={memo.rowStatus === "ARCHIVED"}
          editing={editing}
          pinned={memo.pinned}
          onEdit={() => { setEditContent(memo.content); setEditVisibility(memo.visibility); setEditing(true); }}
          onPin={handleTogglePinned}
          onArchive={handleArchive}
          onRestore={handleRestore}
          onDelete={handlePurge}
        />
      )}
    </div>
  );
}
