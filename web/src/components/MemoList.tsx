import { useState, useEffect, useCallback } from "preact/hooks";
import { api } from "../api";
import { MemoCard } from "./MemoCard";
import type { Memo } from "./MemoCard";
import type { CurrentUser } from "../App";

interface MemoListResponse {
  memos: Memo[];
  nextPageToken: string;
}

interface MemoListProps {
  endpoint: string;
  showEditor: boolean;
  currentUser: CurrentUser | null;
  tag?: string;
  refreshKey?: number;
}

export function MemoList({ endpoint, showEditor, currentUser, tag, refreshKey }: MemoListProps) {
  const [memos, setMemos] = useState<Memo[]>([]);
  const [nextPageToken, setNextPageToken] = useState("");
  const [loading, setLoading] = useState(false);
  const [hasMore, setHasMore] = useState(false);

  const buildUrl = useCallback(
    (pageToken?: string) => {
      const url = new URL(endpoint, window.location.origin);
      if (tag) url.searchParams.set("tag", tag);
      if (pageToken) url.searchParams.set("page_token", pageToken);
      url.searchParams.set("page_size", "20");
      return url.pathname + url.search;
    },
    [endpoint, tag]
  );

  const fetchMemos = useCallback(
    async (pageToken?: string) => {
      setLoading(true);
      try {
        const url = buildUrl(pageToken);
        const data = await api<MemoListResponse>(url);
        if (pageToken) {
          setMemos((prev) => [...prev, ...data.memos]);
        } else {
          setMemos(data.memos);
        }
        setNextPageToken(data.nextPageToken || "");
        setHasMore(!!data.nextPageToken);
      } catch (err) {
        if (!pageToken) setMemos([]);
      } finally {
        setLoading(false);
      }
    },
    [buildUrl]
  );

  useEffect(() => {
    fetchMemos();
  }, [fetchMemos, refreshKey]);

  const handleMemoUpdate = useCallback((updated: Memo) => {
    if (updated.rowStatus === "ARCHIVED") {
      setMemos((prev) => prev.filter((m) => m.uid !== updated.uid));
    } else {
      setMemos((prev) => prev.map((m) => (m.uid === updated.uid ? updated : m)));
    }
  }, []);

  return (
    <div class="memo-list">
      {memos.length === 0 && !loading && (
        <div class="empty-state">
          <div class="empty-state-icon">📝</div>
          暂无备忘录
        </div>
      )}

      {memos.map((memo) => (
        <div key={memo.uid} class="memo-list-item">
          <MemoCard
            memo={memo}
            currentUser={currentUser}
            onUpdate={handleMemoUpdate}
          />
        </div>
      ))}

      {hasMore && (
        <div class="load-more">
          <button
            class="btn btn-secondary"
            onClick={() => fetchMemos(nextPageToken)}
            disabled={loading}
          >
            {loading ? "加载中..." : "加载更多"}
          </button>
        </div>
      )}

      {loading && memos.length === 0 && (
        <div class="loading-screen" style={{ minHeight: "120px" }}>
          <span class="loading-spinner" />
        </div>
      )}
    </div>
  );
}
