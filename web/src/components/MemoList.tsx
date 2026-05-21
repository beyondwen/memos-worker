import { useState, useEffect, useCallback } from "preact/hooks";
import { api, getToken } from "../api";
import { MemoCard } from "./MemoCard";
import type { Memo } from "./MemoCard";
import type { CurrentUser } from "../App";
import { buildMemoListPath, type MemoPropertyFilter, type MemoState, type MemoVisibility } from "../memoQuery";
import { createMemoEventSource, shouldRefreshForSseEvent } from "../sseEvents";

interface MemoListResponse {
  memos: Memo[];
  nextPageToken: string;
}

interface MemoListProps {
  endpoint?: string;
  currentUser: CurrentUser | null;
  tag?: string;
  visibility?: MemoVisibility;
  state?: MemoState;
  search?: string;
  propertyFilter?: MemoPropertyFilter;
  advancedFilter?: string;
  refreshKey?: number;
  emptyText?: string;
}

export function MemoList({
  endpoint = "/api/v1/memos",
  currentUser,
  tag,
  visibility,
  state = "NORMAL",
  search,
  propertyFilter,
  advancedFilter,
  refreshKey,
  emptyText = "暂无备忘录",
}: MemoListProps) {
  const [memos, setMemos] = useState<Memo[]>([]);
  const [nextPageToken, setNextPageToken] = useState("");
  const [loading, setLoading] = useState(false);
  const [hasMore, setHasMore] = useState(false);

  const buildUrl = useCallback(
    (pageToken?: string) => {
      return buildMemoListPath({
        basePath: endpoint,
        tag,
        visibility,
        state,
        search,
        propertyFilter,
        advancedFilter,
        pageToken,
        pageSize: 20,
      });
    },
    [advancedFilter, endpoint, propertyFilter, search, state, tag, visibility]
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

  useEffect(() => {
    if (!currentUser) return;
    const source = createMemoEventSource(getToken());
    if (!source) return;
    const refresh = (message: MessageEvent) => {
      try {
        const event = JSON.parse(message.data);
        if (shouldRefreshForSseEvent(event)) fetchMemos();
      } catch {
        // Ignore malformed SSE payloads.
      }
    };
    for (const type of ["memo.created", "memo.updated", "memo.deleted", "memo.comment.created", "reaction.upserted", "reaction.deleted"]) {
      source.addEventListener(type, refresh);
    }
    return () => source.close();
  }, [currentUser, fetchMemos]);

  const handleMemoUpdate = useCallback((updated: Memo) => {
    if (updated.rowStatus !== state) {
      setMemos((prev) => prev.filter((m) => m.uid !== updated.uid));
    } else {
      setMemos((prev) => prev.map((m) => (m.uid === updated.uid ? updated : m)));
    }
  }, [state]);

  return (
    <div class="memo-list">
      {memos.length === 0 && !loading && (
        <div class="empty-state">
          <div class="empty-state-icon">📝</div>
          {emptyText}
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
