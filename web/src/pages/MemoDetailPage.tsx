import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { useFeedback } from "../components/Feedback";
import { RelationPanel } from "../components/RelationPanel";
import { createMemoEventSource, shouldRefreshForSseEvent } from "../sseEvents";
import type { CurrentUser } from "../App";
import type { Memo } from "../components/MemoCard";
import {
  DetailCommentsSection,
  DetailMemoCard,
  DetailToolbar,
} from "./MemoDetailSections";

interface MemoDetailPageProps {
  path?: string;
  uid?: string;
  currentUser: CurrentUser | null;
}

export function MemoDetailPage({ uid, currentUser }: MemoDetailPageProps) {
  const { notify, confirm } = useFeedback();
  const [memo, setMemo] = useState<Memo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [comments, setComments] = useState<Memo[]>([]);
  const [commentContent, setCommentContent] = useState("");
  const [commenting, setCommenting] = useState(false);

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
    } catch (err) {
      notify(`加载评论失败：${(err as Error).message}`, "error");
    }
  }, [uid]);

  useEffect(() => {
    fetchMemo();
    fetchComments();
  }, [fetchMemo, fetchComments]);

  useEffect(() => {
    if (!currentUser || !uid) return;
    const source = createMemoEventSource();
    if (!source) return;
    const refresh = (message: MessageEvent) => {
      try {
        const event = JSON.parse(message.data);
        if (!shouldRefreshForSseEvent(event) || event.name !== `memos/${uid}`) return;
        fetchMemo();
        fetchComments();
      } catch (err) {
        console.warn("[memo-detail] malformed SSE payload:", err);
      }
    };
    const events = ["memo.updated", "memo.archived", "memo.restored", "memo.deleted", "memo.comment.created"];
    for (const type of events) {
      source.addEventListener(type, refresh);
    }
    return () => source.close();
  }, [currentUser, fetchComments, fetchMemo, uid]);

  const handleArchive = async () => {
    if (!memo) return;
    const ok = await confirm({
      title: "删除这条备忘录？",
      message: "会移入回收站，之后可以恢复或彻底删除。",
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      const data = await api<{ memo: Memo }>(`/api/v1/memos/${memo.uid}`, {
        method: "PATCH",
        body: JSON.stringify({ rowStatus: "ARCHIVED" }),
      });
      setMemo(data.memo);
      notify("备忘录已删除到回收站", "success");
    } catch (err) {
      notify(`删除失败：${(err as Error).message}`, "error");
    }
  };

  const handleRestore = async () => {
    if (!memo) return;
    try {
      const data = await api<{ memo: Memo }>(`/api/v1/memos/${memo.uid}`, {
        method: "PATCH",
        body: JSON.stringify({ rowStatus: "NORMAL" }),
      });
      setMemo(data.memo);
      notify("备忘录已恢复", "success");
    } catch (err) {
      notify(`恢复失败：${(err as Error).message}`, "error");
    }
  };

  const handlePurge = async () => {
    if (!memo) return;
    const ok = await confirm({
      title: "彻底删除这条备忘录？",
      message: "这会永久移除备忘录，附件会解绑但不会一并删除。",
      confirmText: "彻底删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await api(`/api/v1/memos/${memo.uid}?purge=true`, { method: "DELETE" });
      notify("备忘录已彻底删除", "success");
      route("/");
    } catch (err) {
      notify(`彻底删除失败：${(err as Error).message}`, "error");
    }
  };

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

  const isOwner = currentUser?.id === memo.creator.id;
  const isArchived = memo.rowStatus === "ARCHIVED";
  const backHome = (event: Event) => {
    event.preventDefault();
    route("/");
  };

  return (
    <div class="memo-detail-page">
      <DetailToolbar
        memo={memo}
        isOwner={!!isOwner}
        isArchived={isArchived}
        onArchive={handleArchive}
        onRestore={handleRestore}
        onPurge={handlePurge}
        onBackHome={backHome}
      />

      <DetailMemoCard
        memo={memo}
      />

      <RelationPanel memoUid={memo.uid} canEdit={currentUser?.id === memo.creator.id} />

      <DetailCommentsSection
        comments={comments}
        currentUser={currentUser}
        commentContent={commentContent}
        commenting={commenting}
        onCommentContentChange={setCommentContent}
        onAddComment={handleAddComment}
      />
    </div>
  );
}
