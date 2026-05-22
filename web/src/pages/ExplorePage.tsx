import { useState } from "preact/hooks";
import { MemoList } from "../components/MemoList";
import { CustomSelect } from "../components/CustomSelect";
import type { CurrentUser } from "../App";
import type { MemoPropertyFilter } from "../memoQuery";

interface ExplorePageProps {
  path: string;
  currentUser: CurrentUser | null;
}

const PROPERTY_OPTIONS: Array<{ value: MemoPropertyFilter | ""; label: string }> = [
  { value: "", label: "全部类型" },
  { value: "has_task_list", label: "任务" },
  { value: "has_incomplete_tasks", label: "未完成任务" },
  { value: "has_link", label: "链接" },
  { value: "has_code", label: "代码" },
];

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
          <CustomSelect
            value={propertyFilter}
            options={PROPERTY_OPTIONS}
            onChange={setPropertyFilter}
            ariaLabel="内容类型筛选"
          />
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
