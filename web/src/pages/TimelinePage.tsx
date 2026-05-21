import { useCallback, useEffect, useState } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import type { CurrentUser } from "../App";

interface TimelinePageProps {
  path: string;
  currentUser: CurrentUser | null;
}

interface TimelineDay {
  day: string;
  count: number;
}

export function TimelinePage({ currentUser }: TimelinePageProps) {
  const [days, setDays] = useState<TimelineDay[]>([]);

  const fetchTimeline = useCallback(async () => {
    if (!currentUser) return;
    const data = await api<{ days: TimelineDay[] }>("/api/v1/timeline");
    setDays(data.days);
  }, [currentUser]);

  useEffect(() => {
    fetchTimeline().catch(() => undefined);
  }, [fetchTimeline]);

  if (!currentUser) {
    route("/auth", true);
    return null;
  }

  return (
    <div class="settings-layout">
      <div class="home-toolbar page-toolbar">
        <div>
          <div class="home-kicker">Timeline</div>
          <h1>时间线</h1>
          <p>按日期回顾备忘录密度</p>
        </div>
      </div>
      <div class="settings-section">
        <div class="timeline-list">
          {days.map((item) => (
            <button
              key={item.day}
              class="timeline-row"
              onClick={() => route(`/?createdAfter=${item.day}&createdBefore=${item.day}`)}
            >
              <span>{item.day}</span>
              <strong>{item.count}</strong>
            </button>
          ))}
          {days.length === 0 && <div class="muted-line">暂无时间线数据。</div>}
        </div>
      </div>
    </div>
  );
}
