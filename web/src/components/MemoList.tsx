import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { api } from "../api";
import type { Memo } from "./MemoCard";
import type { CurrentUser } from "../App";
import { buildMemoListPath, type MemoPropertyFilter, type MemoState, type MemoVisibility } from "../memoQuery";
import { createMemoEventSource, shouldRefreshForSseEvent } from "../sseEvents";
import { scheduleDebouncedRefresh } from "../sseRefresh";
import { buildBulkMemoRequest, bulkMemoActionLabel, type BulkMemoAction } from "../bulkActions";
import { shouldAutoLoadNextMemoPage } from "../memoListPaging";
import { useFeedback } from "./Feedback";
import { scoreSearchMatch } from "../searchResultView";
import { BulkBar, EmptyMemoList, MemoListItemView, MemoListLoading } from "./MemoListSections";

interface MemoListResponse {
  memos: Memo[];
  nextPageToken: string;
}

interface MemoListMembership {
  state: MemoState;
  visibility?: MemoVisibility;
  tag?: string;
  search?: string;
  propertyFilter?: MemoPropertyFilter;
  advancedFilter?: string;
  createdAfter?: string;
  createdBefore?: string;
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
  createdAfter?: string;
  createdBefore?: string;
  createdMemo?: Memo | null;
  onMemoChanged?: (memo: Memo) => void;
  emptyText?: string;
}

function memoBelongsInList(memo: Memo, filters: MemoListMembership): boolean {
  if (memo.rowStatus !== filters.state) return false;
  if (filters.visibility && memo.visibility !== filters.visibility) return false;
  if (filters.tag && !(memo.payload?.tags ?? []).includes(filters.tag)) return false;
  if (filters.search && !memoMatchesSearch(memo, filters.search)) return false;
  if (filters.propertyFilter && !memoMatchesProperty(memo, filters.propertyFilter)) return false;
  if (filters.advancedFilter) return false;
  const createdDay = memoDateKey(memo.createdTs);
  if (filters.createdAfter && createdDay < filters.createdAfter) return false;
  if (filters.createdBefore && createdDay > filters.createdBefore) return false;
  return true;
}

function memoMatchesSearch(memo: Memo, search: string): boolean {
  const needle = search.trim().toLowerCase();
  if (!needle) return true;
  return memo.content.toLowerCase().includes(needle)
    || (memo.payload?.tags ?? []).some((tag) => tag.toLowerCase().includes(needle));
}

function memoMatchesProperty(memo: Memo, propertyFilter: MemoPropertyFilter): boolean {
  const property = (memo.payload as { property?: Record<string, boolean> })?.property ?? {};
  const keyMap: Record<MemoPropertyFilter, string> = {
    has_task_list: "hasTaskList",
    has_incomplete_tasks: "hasIncompleteTasks",
    has_link: "hasLink",
    has_code: "hasCode",
  };
  return !!property[keyMap[propertyFilter]];
}

function memoDateKey(ts: number): string {
  const date = new Date(ts * 1000);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function compareMemoOrder(a: Memo, b: Memo): number {
  if (Number(a.pinned) !== Number(b.pinned)) return Number(b.pinned) - Number(a.pinned);
  if (a.createdTs !== b.createdTs) return b.createdTs - a.createdTs;
  return b.id - a.id;
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
  createdAfter,
  createdBefore,
  createdMemo,
  onMemoChanged,
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
  const sseRefreshTimer = useRef<number | null>(null);
  const locallyUpdatedNames = useRef<Set<string>>(new Set());
  const loadMoreRef = useRef<HTMLDivElement | null>(null);
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
        createdAfter,
        createdBefore,
        pageToken,
        pageSize: 20,
      });
    },
    [advancedFilter, createdAfter, createdBefore, endpoint, propertyFilter, search, state, tag, visibility]
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
  }, [fetchMemos]);

  useEffect(() => {
    if (!createdMemo) return;
    locallyUpdatedNames.current.add(createdMemo.name);
    if (!memoBelongsInList(createdMemo, { state, visibility, tag, search, propertyFilter, advancedFilter, createdAfter, createdBefore })) return;
    setMemos((prev) => {
      const withoutDuplicate = prev.filter((memo) => memo.uid !== createdMemo.uid);
      return [createdMemo, ...withoutDuplicate].sort(compareMemoOrder);
    });
  }, [advancedFilter, createdAfter, createdBefore, createdMemo, propertyFilter, search, state, tag, visibility]);

  const loadNextPage = useCallback(() => {
    if (!shouldAutoLoadNextMemoPage({ hasMore, loading, nextPageToken })) return;
    fetchMemos(nextPageToken);
  }, [fetchMemos, hasMore, loading, nextPageToken]);

  useEffect(() => {
    const target = loadMoreRef.current;
    if (!target || !hasMore) return;

    if (!("IntersectionObserver" in window)) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) loadNextPage();
    }, { rootMargin: "280px 0px" });
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasMore, loadNextPage]);

  useEffect(() => {
    setSelectedUids((prev) => {
      const visible = new Set(memos.map((memo) => memo.uid));
      const next = new Set([...prev].filter((uid) => visible.has(uid)));
      return next.size === prev.size ? prev : next;
    });
  }, [memos]);

  useEffect(() => {
    if (!currentUser) return;
    const source = createMemoEventSource();
    if (!source) return;
    const refresh = (message: MessageEvent) => {
      try {
        const event = JSON.parse(message.data);
        if (shouldRefreshForSseEvent(event)) {
          if (event.name && locallyUpdatedNames.current.delete(event.name)) return;
          sseRefreshTimer.current = scheduleDebouncedRefresh(
            sseRefreshTimer.current,
            window.setTimeout,
            window.clearTimeout,
            () => {
              sseRefreshTimer.current = null;
              fetchMemos();
            },
          );
        }
      } catch (err) {
        console.warn("[memo-list] malformed SSE payload:", err);
      }
    };
    for (const type of ["memo.created", "memo.updated", "memo.archived", "memo.restored", "memo.deleted", "memo.bulk.updated", "memo.comment.created"]) {
      source.addEventListener(type, refresh);
    }
    return () => {
      if (sseRefreshTimer.current !== null) {
        window.clearTimeout(sseRefreshTimer.current);
        sseRefreshTimer.current = null;
      }
      source.close();
    };
  }, [currentUser, fetchMemos]);

  const handleMemoUpdate = useCallback((updated: Memo) => {
    locallyUpdatedNames.current.add(updated.name);
    onMemoChanged?.(updated);
    if (!memoBelongsInList(updated, { state, visibility, tag, search, propertyFilter, advancedFilter, createdAfter, createdBefore })) {
      setMemos((prev) => prev.filter((m) => m.uid !== updated.uid));
    } else {
      setMemos((prev) => prev.map((m) => (m.uid === updated.uid ? updated : m)).sort(compareMemoOrder));
    }
  }, [advancedFilter, createdAfter, createdBefore, onMemoChanged, propertyFilter, search, state, tag, visibility]);

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
    <div class={`memo-list${selectedUids.size > 0 ? " has-bulk-actions" : ""}`}>
      <BulkBar
        selectableCount={selectableUids.length}
        selectedCount={selectedUids.size}
        allSelected={allSelected}
        state={state}
        bulkVisibility={bulkVisibility}
        bulkWorking={bulkWorking}
        onToggleAll={toggleAll}
        onRunBulkAction={runBulkAction}
        onBulkVisibilityChange={setBulkVisibility}
        onClearSelection={() => setSelectedUids(new Set())}
      />

      {memos.length === 0 && !loading && (
        <EmptyMemoList text={emptyText} />
      )}

      {visibleMemos.map((memo) => (
        <MemoListItemView
          key={memo.uid}
          memo={memo}
          currentUser={currentUser}
          selected={selectedUids.has(memo.uid)}
          selectionMode={selectedUids.size > 0}
          search={search}
          onUpdate={handleMemoUpdate}
          onRemove={handleMemoRemove}
          onSelect={toggleSelected}
          onLongPressStart={startLongPressSelect}
          onLongPressCancel={cancelLongPressSelect}
        />
      ))}

      {hasMore && (
        <div class="load-more" ref={loadMoreRef}>
          <div class="load-more-hint" aria-live="polite">
            {loading ? "正在加载更多..." : "继续向下滚动会自动加载"}
          </div>
          <button
            class="btn btn-secondary"
            onClick={loadNextPage}
            disabled={loading}
          >
            {loading ? "加载中..." : "加载更多"}
          </button>
        </div>
      )}

      {loading && memos.length === 0 && (
        <MemoListLoading />
      )}
    </div>
  );
}
