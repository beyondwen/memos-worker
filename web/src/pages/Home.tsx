import { useState, useEffect, useCallback } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { MemoEditor } from "../components/MemoEditor";
import { MemoList } from "../components/MemoList";
import type { CurrentUser } from "../App";
import type { Memo } from "../components/MemoCard";

interface HomeProps {
  path: string;
  currentUser: CurrentUser | null;
}

export function Home({ currentUser }: HomeProps) {
  const [refreshKey, setRefreshKey] = useState(0);
  const [activeTag, setActiveTag] = useState("");
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

  const endpoint = activeTag
    ? `/api/v1/memos?tag=${encodeURIComponent(activeTag)}`
    : "/api/v1/memos";
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
            <h1>今天</h1>
            <p>{todayLabel}</p>
          </div>
          {activeTag && (
            <button class="tag-clear" onClick={() => setActiveTag("")}>
              清除 #{activeTag}
            </button>
          )}
        </div>

        <MemoEditor onCreated={handleCreated} />
        <MemoList
          key={`${activeTag}-${refreshKey}`}
          endpoint={endpoint}
          showEditor={false}
          currentUser={currentUser}
          tag={activeTag || undefined}
          refreshKey={refreshKey}
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
