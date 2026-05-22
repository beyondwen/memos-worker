import { useCallback, useEffect, useMemo, useState } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import type { CurrentUser } from "../App";
import { CustomSelect } from "../components/CustomSelect";
import {
  DEFAULT_HOLIDAY_COUNTRIES,
  buildCalendarWeeks,
  calendarMonthLabel,
  shiftMonth,
  type CalendarDayCount,
  type CalendarHoliday,
  type HolidayCountry,
} from "../calendarView";
import { buildHomeDateFilterPath } from "../homeFilters";

interface TimelinePageProps {
  path: string;
  currentUser: CurrentUser | null;
}

export function TimelinePage({ currentUser }: TimelinePageProps) {
  const now = useMemo(() => new Date(), []);
  const [year, setYear] = useState(now.getFullYear());
  const [month, setMonth] = useState(now.getMonth() + 1);
  const [country, setCountry] = useState("CN");
  const [countries, setCountries] = useState<HolidayCountry[]>(DEFAULT_HOLIDAY_COUNTRIES);
  const [days, setDays] = useState<CalendarDayCount[]>([]);
  const [holidays, setHolidays] = useState<CalendarHoliday[]>([]);
  const [loading, setLoading] = useState(false);
  const [holidayError, setHolidayError] = useState("");

  useEffect(() => {
    if (!currentUser) return;
    api<{ countries: HolidayCountry[] }>("/api/v1/calendar/countries")
      .then((data) => {
        if (data.countries.length > 0) {
          setCountries(data.countries.sort((a, b) => a.name.localeCompare(b.name)));
        }
      })
      .catch(() => undefined);
  }, [currentUser]);

  const fetchCalendar = useCallback(async () => {
    if (!currentUser) return;
    setLoading(true);
    setHolidayError("");
    try {
      const [timeline, holidayData] = await Promise.all([
        api<{ days: CalendarDayCount[] }>(`/api/v1/timeline?year=${year}&month=${month}`),
        api<{ holidays: CalendarHoliday[] }>(`/api/v1/calendar/holidays?year=${year}&country=${country}`),
      ]);
      setDays(timeline.days);
      setHolidays(holidayData.holidays);
    } catch (err) {
      setHolidayError((err as Error).message);
      const timeline = await api<{ days: CalendarDayCount[] }>(`/api/v1/timeline?year=${year}&month=${month}`);
      setDays(timeline.days);
      setHolidays([]);
    } finally {
      setLoading(false);
    }
  }, [country, currentUser, month, year]);

  useEffect(() => {
    fetchCalendar().catch(() => undefined);
  }, [fetchCalendar]);

  if (!currentUser) {
    route("/auth", true);
    return null;
  }

  const weeks = buildCalendarWeeks(year, month, days, holidays, now);
  const selectedCountry = countries.find((item) => item.countryCode === country);
  const monthMemoCount = days.reduce((total, item) => total + item.count, 0);

  const moveMonth = (delta: number) => {
    const next = shiftMonth(year, month, delta);
    setYear(next.year);
    setMonth(next.month);
  };

  const jumpToday = () => {
    const today = new Date();
    setYear(today.getFullYear());
    setMonth(today.getMonth() + 1);
  };

  return (
    <div class="settings-layout calendar-page">
      <div class="home-toolbar page-toolbar">
        <div>
          <div class="home-kicker">Calendar</div>
          <h1>日历</h1>
          <p>{selectedCountry?.name || country} 假期、世界纪念日和备忘录分布</p>
        </div>
        <div class="calendar-toolbar">
          <button class="btn btn-secondary btn-sm" onClick={() => moveMonth(-1)}>上个月</button>
          <button class="btn btn-ghost btn-sm" onClick={jumpToday}>今天</button>
          <button class="btn btn-secondary btn-sm" onClick={() => moveMonth(1)}>下个月</button>
        </div>
      </div>

      <div class="settings-section calendar-shell">
        <div class="calendar-controls">
          <div>
            <div class="calendar-title">{calendarMonthLabel(year, month)}</div>
            <div class="settings-record-meta">{monthMemoCount} 条备忘录 · {holidays.length} 个国家/地区假期</div>
          </div>
          <div class="calendar-country-select">
            <span>国家/地区</span>
            <CustomSelect
              value={country}
              options={countries.map((item) => ({ value: item.countryCode, label: `${item.name} (${item.countryCode})` }))}
              onChange={setCountry}
              ariaLabel="国家/地区"
            />
          </div>
        </div>

        {holidayError && <div class="inline-message error">假期数据加载失败：{holidayError}</div>}

        <div class="calendar-grid" aria-busy={loading}>
          {["日", "一", "二", "三", "四", "五", "六"].map((day) => (
            <div key={day} class="calendar-weekday">{day}</div>
          ))}
          {weeks.flatMap((week) => week.days).map((day) => (
            <button
              key={day.date}
              class={`calendar-day${day.inMonth ? "" : " muted"}${day.isToday ? " today" : ""}${day.memoCount > 0 ? " has-memo" : ""}`}
              onClick={() => route(buildHomeDateFilterPath(day.date))}
            >
              <span class="calendar-day-number">{day.dayOfMonth}</span>
              {day.memoCount > 0 && <span class="calendar-memo-count">{day.memoCount}</span>}
              <div class="calendar-events">
                {day.events.slice(0, 3).map((event) => (
                  <span key={`${event.kind}-${event.label}`} class={`calendar-event ${event.kind}`} title={event.detail || event.label}>
                    {event.label}
                  </span>
                ))}
                {day.events.length > 3 && <span class="calendar-event more">+{day.events.length - 3}</span>}
              </div>
            </button>
          ))}
        </div>
        <div class="calendar-legend">
          <span><i class="legend-dot memo" />备忘录</span>
          <span><i class="legend-dot holiday" />国家/地区假期</span>
          <span><i class="legend-dot observance" />世界纪念日</span>
          <span class="settings-record-meta">点击日期可筛选当天备忘录。</span>
        </div>
      </div>
    </div>
  );
}
