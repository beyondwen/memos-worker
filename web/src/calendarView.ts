export interface CalendarDayCount {
  day: string;
  count: number;
}

export interface HolidayCountry {
  countryCode: string;
  name: string;
}

export interface CalendarHoliday {
  date: string;
  localName: string;
  name: string;
  countryCode?: string;
  types?: string[];
}

export interface CalendarEvent {
  date: string;
  label: string;
  kind: "holiday" | "observance";
  detail?: string;
}

export interface CalendarDay {
  date: string;
  dayOfMonth: number;
  inMonth: boolean;
  isToday: boolean;
  memoCount: number;
  events: CalendarEvent[];
}

export interface CalendarWeek {
  days: CalendarDay[];
}

export const DEFAULT_HOLIDAY_COUNTRIES: HolidayCountry[] = [
  { countryCode: "CN", name: "China" },
  { countryCode: "US", name: "United States" },
  { countryCode: "JP", name: "Japan" },
  { countryCode: "KR", name: "South Korea" },
  { countryCode: "GB", name: "United Kingdom" },
  { countryCode: "DE", name: "Germany" },
  { countryCode: "FR", name: "France" },
  { countryCode: "CA", name: "Canada" },
  { countryCode: "AU", name: "Australia" },
  { countryCode: "SG", name: "Singapore" },
];

const WORLD_OBSERVANCES: Array<{ month: number; day: number; label: string; detail: string }> = [
  { month: 1, day: 1, label: "世界和平日", detail: "Global Family Day" },
  { month: 1, day: 27, label: "国际大屠杀纪念日", detail: "International Holocaust Remembrance Day" },
  { month: 3, day: 8, label: "国际妇女节", detail: "International Women's Day" },
  { month: 3, day: 20, label: "国际幸福日", detail: "International Day of Happiness" },
  { month: 3, day: 22, label: "世界水日", detail: "World Water Day" },
  { month: 4, day: 7, label: "世界卫生日", detail: "World Health Day" },
  { month: 4, day: 22, label: "世界地球日", detail: "Earth Day" },
  { month: 5, day: 1, label: "国际劳动节", detail: "International Workers' Day" },
  { month: 5, day: 8, label: "世界红十字日", detail: "World Red Cross and Red Crescent Day" },
  { month: 5, day: 18, label: "国际博物馆日", detail: "International Museum Day" },
  { month: 6, day: 1, label: "国际儿童节", detail: "International Children's Day" },
  { month: 6, day: 5, label: "世界环境日", detail: "World Environment Day" },
  { month: 6, day: 21, label: "国际瑜伽日", detail: "International Day of Yoga" },
  { month: 7, day: 18, label: "曼德拉国际日", detail: "Nelson Mandela International Day" },
  { month: 8, day: 19, label: "世界人道主义日", detail: "World Humanitarian Day" },
  { month: 9, day: 21, label: "国际和平日", detail: "International Day of Peace" },
  { month: 10, day: 1, label: "国际老年人日", detail: "International Day of Older Persons" },
  { month: 10, day: 10, label: "世界精神卫生日", detail: "World Mental Health Day" },
  { month: 10, day: 24, label: "联合国日", detail: "United Nations Day" },
  { month: 11, day: 20, label: "世界儿童日", detail: "World Children's Day" },
  { month: 12, day: 3, label: "国际残疾人日", detail: "International Day of Persons with Disabilities" },
  { month: 12, day: 10, label: "世界人权日", detail: "Human Rights Day" },
];

export function buildCalendarWeeks(
  year: number,
  month: number,
  counts: CalendarDayCount[],
  holidays: CalendarHoliday[],
  today: Date = new Date(),
): CalendarWeek[] {
  const countMap = new Map(counts.map((item) => [item.day, item.count]));
  const eventMap = buildCalendarEventMap(year, month, holidays);
  const first = new Date(year, month - 1, 1);
  const start = new Date(first);
  start.setDate(first.getDate() - first.getDay());
  const todayKey = formatDateKey(today);
  const weeks: CalendarWeek[] = [];
  for (let week = 0; week < 6; week += 1) {
    const days: CalendarDay[] = [];
    for (let weekday = 0; weekday < 7; weekday += 1) {
      const date = new Date(start);
      date.setDate(start.getDate() + week * 7 + weekday);
      const key = formatDateKey(date);
      days.push({
        date: key,
        dayOfMonth: date.getDate(),
        inMonth: date.getMonth() === month - 1,
        isToday: key === todayKey,
        memoCount: countMap.get(key) ?? 0,
        events: eventMap.get(key) ?? [],
      });
    }
    weeks.push({ days });
  }
  return weeks;
}

export function buildCalendarEventMap(
  year: number,
  month: number,
  holidays: CalendarHoliday[],
): Map<string, CalendarEvent[]> {
  const map = new Map<string, CalendarEvent[]>();
  for (const holiday of holidays) {
    addEvent(map, holiday.date, {
      date: holiday.date,
      label: holiday.localName || holiday.name,
      detail: holiday.name,
      kind: "holiday",
    });
  }
  for (const observance of WORLD_OBSERVANCES.filter((item) => item.month === month)) {
    const date = `${year}-${String(month).padStart(2, "0")}-${String(observance.day).padStart(2, "0")}`;
    addEvent(map, date, {
      date,
      label: observance.label,
      detail: observance.detail,
      kind: "observance",
    });
  }
  return map;
}

function addEvent(map: Map<string, CalendarEvent[]>, date: string, event: CalendarEvent): void {
  const events = map.get(date) ?? [];
  if (!events.some((item) => item.label === event.label && item.kind === event.kind)) {
    events.push(event);
  }
  map.set(date, events);
}

export function calendarMonthLabel(year: number, month: number): string {
  return `${year} 年 ${month} 月`;
}

export function shiftMonth(year: number, month: number, delta: number): { year: number; month: number } {
  const date = new Date(year, month - 1 + delta, 1);
  return { year: date.getFullYear(), month: date.getMonth() + 1 };
}

export function formatDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}
