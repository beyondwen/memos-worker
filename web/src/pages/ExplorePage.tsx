import { useState } from "preact/hooks";
import { MemoList } from "../components/MemoList";
import type { CurrentUser } from "../App";
import type { MemoPropertyFilter } from "../memoQuery";

interface ExplorePageProps {
  path: string;
  currentUser: CurrentUser | null;
}

export function ExplorePage({ currentUser }: ExplorePageProps) {
  const [search, setSearch] = useState("");
  const [propertyFilter, setPropertyFilter] = useState<MemoPropertyFilter | "">("");

  return (
    <div class="layout layout-single">
      <div class="main-content">
        <div class="home-toolbar page-toolbar">
          <div>
            <div class="home-kicker">Explore</div>
            <h1>发现</h1>
            <p>所有用户公开的备忘录</p>
          </div>
        </div>
        <div class="filter-panel">
          <label class="search-box">
            <span>⌕</span>
            <input
              type="search"
              placeholder="搜索公开内容"
              value={search}
              onInput={(e) => setSearch((e.target as HTMLInputElement).value)}
            />
          </label>
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
        </div>
        <MemoList
          key={`${search}-${propertyFilter}`}
          currentUser={currentUser}
          visibility="PUBLIC"
          search={search}
          propertyFilter={propertyFilter || undefined}
          emptyText="暂无公开备忘录"
        />
      </div>
    </div>
  );
}
