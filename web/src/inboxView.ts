export interface InboxItem {
  id: number;
  createdTs: number;
  status: "UNREAD" | "READ";
  sender: { id: number; username?: string; nickname?: string } | null;
  message: { type: string; memoUid?: string; commentUid?: string };
}

export interface InboxDisplay {
  title: string;
  detail: string;
  memoPath: string | null;
}

export function formatInboxItem(item: InboxItem): InboxDisplay {
  const senderName = item.sender?.nickname || item.sender?.username || "有人";
  if (item.message.type === "memo.comment.created") {
    return {
      title: `${senderName} 评论了你的备忘录`,
      detail: "打开备忘录查看回复",
      memoPath: item.message.memoUid ? `/memos/${item.message.memoUid}` : null,
    };
  }
  return {
    title: "新的通知",
    detail: item.message.type || "未知类型",
    memoPath: item.message.memoUid ? `/memos/${item.message.memoUid}` : null,
  };
}
