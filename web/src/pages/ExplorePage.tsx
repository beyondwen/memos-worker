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
    <div class="layout layout-single">
      <div class="main-content">
        <div class="home-toolbar page-toolbar">
          <div>
            <div class="home-kicker">Explore</div>
            <h1>发现</h1>
            <p>所有用户公开的备忘录</p>
          </div>
        </div>
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
