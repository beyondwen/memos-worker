import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { MemoEditor } from "../components/MemoEditor";
import { MemoList } from "../components/MemoList";
import type { CurrentUser } from "../App";
import type { Memo } from "../components/MemoCard";
import type { MemoPropertyFilter, MemoState, MemoVisibility } from "../memoQuery";
import { buildAdvancedMemoFilter, type PinnedFilter } from "../advancedSearch";

interface HomeProps {
  path: string;
  currentUser: CurrentUser | null;
}

export function Home({ currentUser }: HomeProps) {
  const [refreshKey, setRefreshKey] = useState(0);
  const [activeTag, setActiveTag] = useState("");
  const [viewState, setViewState] = useState<MemoState>("NORMAL");
  const [search, setSearch] = useState("");
  const [visibility, setVisibility] = useState<MemoVisibility | "">("");
  const [propertyFilter, setPropertyFilter] = useState<MemoPropertyFilter | "">("");
  const [creator, setCreator] = useState("");
  const [pinnedFilter, setPinnedFilter] = useState<PinnedFilter>("");
  const [createdAfter, setCreatedAfter] = useState("");
  const [createdBefore, setCreatedBefore] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [tags, setTags] = useState<string[]>([]);

  useEffect(() => {
    if (!currentUser) {
      route("/auth", true);
    }
  }, [currentUser]);

  const fetchTags = useCallback(async () => {
    try {
      const data = await api<{ memos: Memo[] }>("/api/v1/memos?page_size=200");
      const tagSet = new Set<string>();
      for (const memo of data.memos) {
        const payload = memo.payload as { tags?: string[] };
        if (payload?.tags) {
          for (const t of payload.tags) tagSet.add(t);
        }
      }
      setTags([...tagSet].sort());
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    fetchTags();
  }, [fetchTags, refreshKey]);

  const handleCreated = () => {
    setRefreshKey((k) => k + 1);
  };

  const handleTagClick = (tag: string) => {
    setActiveTag((prev) => (prev === tag ? "" : tag));
  };

  if (!currentUser) return null;

  const resetFilters = () => {
    setActiveTag("");
    setSearch("");
    setVisibility("");
    setPropertyFilter("");
    setCreator("");
    setPinnedFilter("");
    setCreatedAfter("");
    setCreatedBefore("");
  };

  const advancedFilter = buildAdvancedMemoFilter({ creator, pinned: pinnedFilter, createdAfter, createdBefore });
  const hasFilters = !!activeTag || !!search.trim() || !!visibility || !!propertyFilter || !!advancedFilter;
  const hasSidebar = tags.length > 0;
  const todayLabel = new Date().toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "long",
    day: "numeric",
    weekday: "long",
  });

  return (
    <div class={`layout${hasSidebar ? "" : " layout-single"}`}>
      <div class="main-content">
        <div class="home-toolbar">
          <div>
            <div class="home-kicker">Today</div>
            <h1>{viewState === "ARCHIVED" ? "回收站" : "今天"}</h1>
            <p>{viewState === "ARCHIVED" ? "可恢复或彻底删除的备忘录" : todayLabel}</p>
          </div>
          {hasFilters && (
            <button class="tag-clear" onClick={resetFilters}>
              清除筛选
            </button>
          )}
        </div>

        <div class="filter-panel">
          <div class="view-switch" aria-label="列表范围">
            <button
              class={viewState === "NORMAL" ? "active" : ""}
              onClick={() => setViewState("NORMAL")}
            >
              正常
            </button>
            <button
              class={viewState === "ARCHIVED" ? "active" : ""}
              onClick={() => setViewState("ARCHIVED")}
            >
              回收站
            </button>
          </div>

          <label class="search-box">
            <span>⌕</span>
            <input
              type="search"
              placeholder="搜索内容"
              value={search}
              onInput={(e) => setSearch((e.target as HTMLInputElement).value)}
            />
          </label>

          <select
            class="filter-select"
            value={visibility}
            onChange={(e) => setVisibility((e.target as HTMLSelectElement).value as MemoVisibility | "")}
            aria-label="可见性筛选"
          >
            <option value="">全部可见性</option>
            <option value="PRIVATE">私有</option>
            <option value="PROTECTED">登录可见</option>
            <option value="PUBLIC">公开</option>
          </select>

          <select
            class="filter-select"
            value={propertyFilter}
            onChange={(e) => setPropertyFilter((e.target as HTMLSelectElement).value as MemoPropertyFilter | "")}
            aria-label="内容类型筛选"
          >
            <option value="">全部类型</option>
            <option value="has_task_list">任务</option>
            <option value="has_incomplete_tasks">未完成任务</option>
            <option value="has_link">链接</option>
            <option value="has_code">代码</option>
          </select>

          <button class="btn btn-ghost btn-sm" onClick={() => setShowAdvanced((value) => !value)}>
            {showAdvanced ? "收起高级" : "高级筛选"}
          </button>
        </div>

        {showAdvanced && (
          <div class="advanced-panel">
            <input
              class="form-input"
              type="text"
              placeholder="创建者用户名"
              value={creator}
              onInput={(e) => setCreator((e.target as HTMLInputElement).value)}
            />
            <select
              class="filter-select"
              value={pinnedFilter}
              onChange={(e) => setPinnedFilter((e.target as HTMLSelectElement).value as PinnedFilter)}
            >
              <option value="">全部置顶状态</option>
              <option value="PINNED">仅置顶</option>
              <option value="UNPINNED">未置顶</option>
            </select>
            <label>
              <span>开始日期</span>
              <input
                class="form-input"
                type="date"
                value={createdAfter}
                onInput={(e) => setCreatedAfter((e.target as HTMLInputElement).value)}
              />
            </label>
            <label>
              <span>结束日期</span>
              <input
                class="form-input"
                type="date"
                value={createdBefore}
                onInput={(e) => setCreatedBefore((e.target as HTMLInputElement).value)}
              />
            </label>
          </div>
        )}

        {viewState === "NORMAL" && <MemoEditor onCreated={handleCreated} />}
        <MemoList
          key={`${activeTag}-${viewState}-${search}-${visibility}-${propertyFilter}-${advancedFilter}-${refreshKey}`}
          currentUser={currentUser}
          tag={activeTag || undefined}
          state={viewState}
          search={search}
          visibility={visibility || undefined}
          propertyFilter={propertyFilter || undefined}
          advancedFilter={advancedFilter}
          refreshKey={refreshKey}
          emptyText={viewState === "ARCHIVED" ? "回收站为空" : "暂无备忘录"}
        />
      </div>

      {hasSidebar && (
        <aside class="sidebar">
          <div class="sidebar-section">
            <div class="sidebar-title">标签</div>
            <div class="tag-list">
              {tags.map((tag) => (
                <button
                  key={tag}
                  class={`tag-item${activeTag === tag ? " active" : ""}`}
                  onClick={() => handleTagClick(tag)}
                >
                  #{tag}
                </button>
              ))}
            </div>
          </div>
        </aside>
      )}
    </div>
  );
}
