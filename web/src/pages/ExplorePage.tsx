import { useState } from "preact/hooks";
import { MemoList } from "../components/MemoList";
import type { CurrentUser } from "../App";

interface ExplorePageProps {
  path: string;
  currentUser: CurrentUser | null;
}

export function ExplorePage({ currentUser }: ExplorePageProps) {
  const [activeTag, setActiveTag] = useState("");

  const endpoint = activeTag
    ? `/api/v1/memos?visibility=PUBLIC&tag=${encodeURIComponent(activeTag)}`
    : "/api/v1/memos?visibility=PUBLIC";

  return (
    <div class="layout">
      <div class="main-content">
        <h2 style={{ fontSize: "1.2rem", fontWeight: 600, marginBottom: "4px" }}>
          发现
        </h2>
        <p style={{ color: "var(--text-secondary)", fontSize: "0.9rem", marginBottom: "16px" }}>
          所有用户的公开备忘录
        </p>
        <MemoList
          endpoint={endpoint}
          showEditor={false}
          currentUser={currentUser}
          tag={activeTag || undefined}
        />
      </div>
    </div>
  );
}
