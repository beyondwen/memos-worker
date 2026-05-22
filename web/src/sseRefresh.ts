export const MEMO_LIST_SSE_REFRESH_DEBOUNCE_MS = 400;

type ScheduleTimer = (callback: () => void, delay: number) => number;
type ClearTimer = (timer: number) => void;

export function scheduleDebouncedRefresh(
  pendingTimer: number | null,
  scheduleTimer: ScheduleTimer,
  clearTimer: ClearTimer,
  refresh: () => void,
): number {
  if (pendingTimer !== null) clearTimer(pendingTimer);
  return scheduleTimer(refresh, MEMO_LIST_SSE_REFRESH_DEBOUNCE_MS);
}
