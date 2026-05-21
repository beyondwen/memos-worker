import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { api, getToken } from "../api";
import { MemoCard } from "./MemoCard";
import type { Memo } from "./MemoCard";
import type { CurrentUser } from "../App";
import { buildMemoListPath, type MemoPropertyFilter, type MemoState, type MemoVisibility } from "../memoQuery";
import { createMemoEventSource, shouldRefreshForSseEvent } from "../sseEvents";
import { buildBulkMemoRequest, bulkMemoActionLabel, type BulkMemoAction } from "../bulkActions";
import { useFeedback } from "./Feedback";
import { buildSearchSnippet, scoreSearchMatch } from "../searchResultView";

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
  const [selectedUids, setSelectedUids] = useState<Set<string>>(() => new Set());
  const [bulkVisibility, setBulkVisibility] = useState<MemoVisibility>("PRIVATE");
  const [bulkWorking, setBulkWorking] = useState(false);
  const longPressTimer = useRef<number | null>(null);
  const { notify, confirm } = useFeedback();

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
    setSelectedUids((prev) => {
      const visible = new Set(memos.map((memo) => memo.uid));
      const next = new Set([...prev].filter((uid) => visible.has(uid)));
      return next.size === prev.size ? prev : next;
    });
  }, [memos]);

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
    for (const type of ["memo.created", "memo.updated", "memo.archived", "memo.restored", "memo.deleted", "memo.bulk.updated", "memo.comment.created", "reaction.upserted", "reaction.deleted"]) {
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

  const handleMemoRemove = useCallback((uid: string) => {
    setMemos((prev) => prev.filter((memo) => memo.uid !== uid));
    setSelectedUids((prev) => {
      const next = new Set(prev);
      next.delete(uid);
      return next;
    });
  }, []);

  const toggleSelected = useCallback((uid: string, checked: boolean) => {
    setSelectedUids((prev) => {
      const next = new Set(prev);
      if (checked) next.add(uid);
      else next.delete(uid);
      return next;
    });
  }, []);

  const selectableUids = memos
    .filter((memo) => currentUser && memo.creator.id === currentUser.id)
    .map((memo) => memo.uid);
  const allSelected = selectableUids.length > 0 && selectableUids.every((uid) => selectedUids.has(uid));
  const visibleMemos = search
    ? [...memos].sort((a, b) => scoreSearchMatch({ content: b.content, tags: b.payload.tags }, search) - scoreSearchMatch({ content: a.content, tags: a.payload.tags }, search))
    : memos;

  const toggleAll = useCallback(() => {
    setSelectedUids((prev) => {
      const next = new Set(prev);
      if (allSelected) {
        for (const uid of selectableUids) next.delete(uid);
      } else {
        for (const uid of selectableUids) next.add(uid);
      }
      return next;
    });
  }, [allSelected, selectableUids]);

  const runBulkAction = useCallback(async (action: BulkMemoAction) => {
    const request = buildBulkMemoRequest(action, [...selectedUids], bulkVisibility);
    if (!request.ok) {
      notify(request.error, "info");
      return;
    }

    if (action === "DELETE") {
      const ok = await confirm({
        title: "彻底删除选中的备忘录？",
        message: "这会从回收站中永久移除，附件会解绑但不会一并删除。",
        confirmText: "彻底删除",
        danger: true,
      });
      if (!ok) return;
    }

    setBulkWorking(true);
    try {
      const result = await api<{ updated: number; deleted: number; skipped: number }>("/api/v1/memos/batch", {
        method: "POST",
        body: JSON.stringify(request.body),
      });
      setSelectedUids(new Set());
      await fetchMemos();
      const count = result.deleted || result.updated;
      notify(`${bulkMemoActionLabel(action)} ${count} 条，跳过 ${result.skipped} 条`, "success");
    } catch (err) {
      notify(`${bulkMemoActionLabel(action)}失败：${(err as Error).message}`, "error");
    } finally {
      setBulkWorking(false);
    }
  }, [bulkVisibility, confirm, fetchMemos, notify, selectedUids]);

  const startLongPressSelect = useCallback((memo: Memo) => {
    if (!currentUser || memo.creator.id !== currentUser.id) return;
    if (longPressTimer.current) window.clearTimeout(longPressTimer.current);
    longPressTimer.current = window.setTimeout(() => {
      toggleSelected(memo.uid, true);
      notify("已进入批量选择", "info");
    }, 520);
  }, [currentUser, notify, toggleSelected]);

  const cancelLongPressSelect = useCallback(() => {
    if (longPressTimer.current) {
      window.clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  }, []);

  return (
    <div class="memo-list">
      {selectableUids.length > 0 && (
        <div class={`bulk-bar${selectedUids.size > 0 ? " active" : ""}`}>
          <label class="bulk-select-all">
            <input
              type="checkbox"
              checked={allSelected}
              onChange={toggleAll}
              aria-label="选择当前页可操作备忘录"
            />
            <span>{selectedUids.size > 0 ? `已选 ${selectedUids.size} 条` : "批量选择"}</span>
          </label>

          {selectedUids.size > 0 && (
            <div class="bulk-actions">
              {state === "NORMAL" ? (
                <button class="btn btn-secondary btn-sm" onClick={() => runBulkAction("ARCHIVE")} disabled={bulkWorking}>
                  归档
                </button>
              ) : (
                <>
                  <button class="btn btn-secondary btn-sm" onClick={() => runBulkAction("RESTORE")} disabled={bulkWorking}>
                    恢复
                  </button>
                  <button class="btn btn-danger btn-sm" onClick={() => runBulkAction("DELETE")} disabled={bulkWorking}>
                    彻底删除
                  </button>
                </>
              )}
              <select
                class="filter-select compact"
                value={bulkVisibility}
                onChange={(e) => setBulkVisibility((e.target as HTMLSelectElement).value as MemoVisibility)}
                disabled={bulkWorking}
              >
                <option value="PRIVATE">私有</option>
                <option value="PROTECTED">登录可见</option>
                <option value="PUBLIC">公开</option>
              </select>
              <button class="btn btn-secondary btn-sm" onClick={() => runBulkAction("VISIBILITY")} disabled={bulkWorking}>
                改可见性
              </button>
              <button class="btn btn-ghost btn-sm" onClick={() => setSelectedUids(new Set())} disabled={bulkWorking}>
                清空
              </button>
            </div>
          )}
        </div>
      )}

      {memos.length === 0 && !loading && (
        <div class="empty-state">
          <div class="empty-state-icon">📝</div>
          {emptyText}
        </div>
      )}

      {visibleMemos.map((memo) => (
        <div
          key={memo.uid}
          class={`memo-list-item${selectedUids.has(memo.uid) ? " selected" : ""}`}
          onTouchStart={() => startLongPressSelect(memo)}
          onTouchEnd={cancelLongPressSelect}
          onTouchMove={cancelLongPressSelect}
          onTouchCancel={cancelLongPressSelect}
        >
          {currentUser && memo.creator.id === currentUser.id && (
            <label class="memo-select">
              <input
                type="checkbox"
                checked={selectedUids.has(memo.uid)}
                onChange={(e) => toggleSelected(memo.uid, (e.target as HTMLInputElement).checked)}
                aria-label="选择备忘录"
              />
            </label>
          )}
          <MemoCard
            memo={memo}
            currentUser={currentUser}
            onUpdate={handleMemoUpdate}
            onRemove={handleMemoRemove}
            highlight={search}
          />
          {search && (
            <div class="search-snippet">
              {buildSearchSnippet(memo.content, search, 44)}
            </div>
          )}
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
