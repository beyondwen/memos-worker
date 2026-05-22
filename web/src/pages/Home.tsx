import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { MemoEditor } from "../components/MemoEditor";
import { MemoList } from "../components/MemoList";
import { CustomSelect } from "../components/CustomSelect";
import { DatePicker } from "../components/DateTimePicker";
import type { CurrentUser } from "../App";
import type { Memo } from "../components/MemoCard";
import type { MemoPropertyFilter, MemoState, MemoVisibility } from "../memoQuery";
import { buildAdvancedMemoFilter, type PinnedFilter } from "../advancedSearch";
import { parseHomeDateFilterParams, stripHomeFilterParams } from "../homeFilters";

interface HomeProps {
  path: string;
  currentUser: CurrentUser | null;
}

const VISIBILITY_OPTIONS: Array<{ value: MemoVisibility | ""; label: string }> = [
  { value: "", label: "全部可见性" },
  { value: "PRIVATE", label: "私有" },
  { value: "PROTECTED", label: "登录可见" },
  { value: "PUBLIC", label: "公开" },
];

const PROPERTY_OPTIONS: Array<{ value: MemoPropertyFilter | ""; label: string }> = [
  { value: "", label: "全部类型" },
  { value: "has_task_list", label: "任务" },
  { value: "has_incomplete_tasks", label: "未完成任务" },
  { value: "has_link", label: "链接" },
  { value: "has_code", label: "代码" },
];

const PINNED_OPTIONS: Array<{ value: PinnedFilter; label: string }> = [
  { value: "", label: "全部置顶状态" },
  { value: "PINNED", label: "仅置顶" },
  { value: "UNPINNED", label: "未置顶" },
];

export function Home({ currentUser }: HomeProps) {
  const [createdMemo, setCreatedMemo] = useState<Memo | null>(null);
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
  const [recentSearches, setRecentSearches] = useState<string[]>(() => {
    try {
      return JSON.parse(localStorage.getItem("memos_recent_searches") || "[]");
    } catch (err) {
      console.warn("[home] recent searches parse failed:", err);
      return [];
    }
  });

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
    } catch (err) {
      console.warn("[home] tag refresh failed:", err);
    }
  }, []);

  useEffect(() => {
    fetchTags();
  }, [fetchTags]);

  useEffect(() => {
    const filters = parseHomeDateFilterParams(window.location.search);
    if (filters.createdAfter || filters.createdBefore) {
      setCreatedAfter(filters.createdAfter);
      setCreatedBefore(filters.createdBefore);
      setShowAdvanced(true);
    }
  }, []);

  const mergeTagsFromMemo = (memo: Memo) => {
    const nextTags = memo.payload?.tags ?? [];
    if (nextTags.length === 0) return;
    setTags((prev) => [...new Set([...prev, ...nextTags])].sort());
  };

  const handleCreated = (memo: Memo) => {
    setCreatedMemo(memo);
    mergeTagsFromMemo(memo);
  };

  const handleMemoChanged = (memo: Memo) => {
    mergeTagsFromMemo(memo);
  };

  const handleTagClick = (tag: string) => {
    setActiveTag((prev) => (prev === tag ? "" : tag));
  };

  const commitSearch = (value: string) => {
    const term = value.trim();
    if (!term) return;
    setRecentSearches((prev) => {
      const next = [term, ...prev.filter((item) => item !== term)].slice(0, 6);
      localStorage.setItem("memos_recent_searches", JSON.stringify(next));
      return next;
    });
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
    const cleanPath = stripHomeFilterParams(`${window.location.pathname}${window.location.search}`);
    if (cleanPath !== `${window.location.pathname}${window.location.search}`) {
      route(cleanPath, true);
    }
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
              aria-label="搜索内容"
              value={search}
              onInput={(e) => setSearch((e.target as HTMLInputElement).value)}
              onKeyDown={(e) => { if (e.key === "Enter") commitSearch(search); }}
              onBlur={() => commitSearch(search)}
            />
          </label>

          <CustomSelect
            value={visibility}
            options={VISIBILITY_OPTIONS}
            onChange={setVisibility}
            ariaLabel="可见性筛选"
          />

          <CustomSelect
            value={propertyFilter}
            options={PROPERTY_OPTIONS}
            onChange={setPropertyFilter}
            ariaLabel="内容类型筛选"
          />

          <button class="btn btn-ghost btn-sm" onClick={() => setShowAdvanced((value) => !value)}>
            {showAdvanced ? "收起高级" : "高级筛选"}
          </button>
        </div>

        {recentSearches.length > 0 && (
          <div class="recent-searches" aria-label="最近搜索">
            <span class="recent-searches-label">最近搜索</span>
            {recentSearches.map((term) => (
              <button key={term} class="recent-search-chip" onClick={() => setSearch(term)}>
                <span aria-hidden="true">↺</span>
                {term}
              </button>
            ))}
          </div>
        )}

        {showAdvanced && (
          <div class="advanced-panel">
            <input
              class="form-input"
              type="text"
              placeholder="创建者用户名"
              aria-label="创建者用户名"
              value={creator}
              onInput={(e) => setCreator((e.target as HTMLInputElement).value)}
            />
            <CustomSelect
              value={pinnedFilter}
              options={PINNED_OPTIONS}
              onChange={setPinnedFilter}
              ariaLabel="置顶状态筛选"
            />
            <DatePicker value={createdAfter} onChange={setCreatedAfter} placeholder="开始日期" />
            <DatePicker value={createdBefore} onChange={setCreatedBefore} placeholder="结束日期" />
          </div>
        )}

        {viewState === "NORMAL" && <MemoEditor onCreated={handleCreated} />}
        <MemoList
          key={`${activeTag}-${viewState}-${search}-${visibility}-${propertyFilter}-${advancedFilter}-${createdAfter}-${createdBefore}`}
          currentUser={currentUser}
          tag={activeTag || undefined}
          state={viewState}
          search={search}
          visibility={visibility || undefined}
          propertyFilter={propertyFilter || undefined}
          advancedFilter={advancedFilter}
          createdAfter={createdAfter}
          createdBefore={createdBefore}
          createdMemo={createdMemo}
          onMemoChanged={handleMemoChanged}
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
