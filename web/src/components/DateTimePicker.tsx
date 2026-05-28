import { createPortal } from "preact/compat";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "preact/hooks";
import { buildDateTimeLocal, formatDateTimeLocalLabel, nowDateTimeLocal } from "../richText";

interface DateTimePickerProps {
  value: string;
  onChange: (value: string) => void;
}

interface DatePickerProps {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
}

interface CalendarDay {
  key: string;
  day: number;
  inMonth: boolean;
  isSelected: boolean;
  isToday: boolean;
}

const POPOVER_WIDTH = 288;
const POPOVER_MARGIN = 12;
const POPOVER_GAP = 8;

export function DateTimePicker({ value, onChange }: DateTimePickerProps) {
  const selected = parseDateTime(value);
  const [open, setOpen] = useState(false);
  const [view, setView] = useState(() => new Date(selected.year, selected.month - 1, 1));
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const popoverPosition = useFloatingPopover(open, triggerRef, popoverRef);
  const days = useMemo(() => buildCalendarDays(view, selected.dateKey), [selected.dateKey, view]);

  useDismiss(open, [rootRef, popoverRef], () => setOpen(false));

  const chooseDay = (key: string) => {
    const [year, month, day] = key.split("-").map(Number);
    onChange(buildDateTimeLocal(year, month, day, selected.hour, selected.minute));
  };

  const updateTime = (hour: number, minute: number) => {
    onChange(buildDateTimeLocal(selected.year, selected.month, selected.day, hour, minute));
  };

  const setNow = () => {
    const next = nowDateTimeLocal();
    onChange(next);
    setView(monthStart(new Date(next)));
  };

  return (
    <div ref={rootRef} class="date-picker">
      <button ref={triggerRef} type="button" class="date-picker-trigger" onClick={() => setOpen((value) => !value)} aria-expanded={open}>
        <span>日期</span>
        <strong>{formatDateTimeLocalLabel(value)}</strong>
        <span aria-hidden="true">⌄</span>
      </button>
      {open && popoverPosition && createPortal(
        <div
          ref={popoverRef}
          class="date-popover"
          style={{ left: `${popoverPosition.left}px`, top: `${popoverPosition.top}px` }}
        >
          <CalendarHeader view={view} setView={setView} />
          <CalendarGrid days={days} chooseDay={chooseDay} />
          <div class="time-row">
            <label>
              <span>小时</span>
              <input type="number" min="0" max="23" value={String(selected.hour).padStart(2, "0")} onInput={(event) => updateTime(clampTime(event, 23), selected.minute)} />
            </label>
            <label>
              <span>分钟</span>
              <input type="number" min="0" max="59" value={String(selected.minute).padStart(2, "0")} onInput={(event) => updateTime(selected.hour, clampTime(event, 59))} />
            </label>
          </div>
          <div class="date-popover-actions">
            <button type="button" onClick={setNow}>现在</button>
            <button type="button" onClick={() => setOpen(false)}>完成</button>
          </div>
        </div>,
        document.body
      )}
    </div>
  );
}

export function DatePicker({ value, onChange, placeholder }: DatePickerProps) {
  const parsed = parseDateOnly(value);
  const [open, setOpen] = useState(false);
  const [view, setView] = useState(() => new Date(parsed.year, parsed.month - 1, 1));
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const popoverPosition = useFloatingPopover(open, triggerRef, popoverRef);
  const days = useMemo(() => buildCalendarDays(view, value), [value, view]);

  useDismiss(open, [rootRef, popoverRef], () => setOpen(false));

  return (
    <div ref={rootRef} class="date-picker date-only-picker">
      <button ref={triggerRef} type="button" class="date-picker-trigger" onClick={() => setOpen((next) => !next)} aria-expanded={open}>
        <span>{placeholder}</span>
        <strong>{value || "未选择"}</strong>
        <span aria-hidden="true">⌄</span>
      </button>
      {open && popoverPosition && createPortal(
        <div
          ref={popoverRef}
          class="date-popover"
          style={{ left: `${popoverPosition.left}px`, top: `${popoverPosition.top}px` }}
        >
          <CalendarHeader view={view} setView={setView} />
          <CalendarGrid
            days={days}
            chooseDay={(key) => {
              onChange(key);
              setOpen(false);
            }}
          />
          <div class="date-popover-actions">
            <button type="button" onClick={() => onChange("")}>清除</button>
            <button type="button" onClick={() => setOpen(false)}>完成</button>
          </div>
        </div>,
        document.body
      )}
    </div>
  );
}

function CalendarHeader({ view, setView }: { view: Date; setView: (date: Date) => void }) {
  return (
    <div class="date-popover-header">
      <button type="button" onClick={() => setView(new Date(view.getFullYear(), view.getMonth() - 1, 1))}>‹</button>
      <strong>{view.getFullYear()}年{String(view.getMonth() + 1).padStart(2, "0")}月</strong>
      <button type="button" onClick={() => setView(new Date(view.getFullYear(), view.getMonth() + 1, 1))}>›</button>
    </div>
  );
}

function CalendarGrid({ days, chooseDay }: { days: CalendarDay[]; chooseDay: (key: string) => void }) {
  return (
    <div class="mini-calendar-grid">
      {["日", "一", "二", "三", "四", "五", "六"].map((day) => (
        <span key={day} class="mini-calendar-weekday">{day}</span>
      ))}
      {days.map((day) => (
        <button
          key={day.key}
          type="button"
          class={`${day.inMonth ? "" : "muted"}${day.isSelected ? " selected" : ""}${day.isToday ? " today" : ""}`}
          onClick={() => chooseDay(day.key)}
        >
          {day.day}
        </button>
      ))}
    </div>
  );
}

function useFloatingPopover(
  open: boolean,
  triggerRef: { current: HTMLElement | null },
  popoverRef: { current: HTMLElement | null },
) {
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    if (!open) {
      setPosition(null);
      return;
    }

    let frame = 0;
    const update = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const trigger = triggerRef.current;
        if (!trigger) return;

        const triggerRect = trigger.getBoundingClientRect();
        const width = popoverRef.current?.offsetWidth || POPOVER_WIDTH;
        const height = popoverRef.current?.offsetHeight || 320;
        const maxLeft = Math.max(POPOVER_MARGIN, window.innerWidth - width - POPOVER_MARGIN);
        const left = Math.max(POPOVER_MARGIN, Math.min(triggerRect.right - width, maxLeft));
        const belowTop = triggerRect.bottom + POPOVER_GAP;
        const aboveTop = triggerRect.top - height - POPOVER_GAP;
        const fitsBelow = belowTop + height <= window.innerHeight - POPOVER_MARGIN;
        const top = fitsBelow || aboveTop < POPOVER_MARGIN ? belowTop : aboveTop;

        setPosition({
          left,
          top: Math.max(POPOVER_MARGIN, Math.min(top, window.innerHeight - height - POPOVER_MARGIN)),
        });
      });
    };

    update();
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [open, popoverRef, triggerRef]);

  return position;
}

function useDismiss(open: boolean, refs: Array<{ current: HTMLElement | null }>, close: () => void) {
  useEffect(() => {
    if (!open) return;
    const onPointer = (event: MouseEvent) => {
      const target = event.target as Node;
      if (refs.some((ref) => ref.current?.contains(target))) return;
      close();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [close, open, refs]);
}

function buildCalendarDays(view: Date, selectedKey: string): CalendarDay[] {
  const first = new Date(view.getFullYear(), view.getMonth(), 1);
  const start = new Date(first);
  start.setDate(first.getDate() - first.getDay());
  const today = formatDateKey(new Date());
  return Array.from({ length: 42 }, (_, index) => {
    const date = new Date(start);
    date.setDate(start.getDate() + index);
    const key = formatDateKey(date);
    return {
      key,
      day: date.getDate(),
      inMonth: date.getMonth() === view.getMonth(),
      isSelected: key === selectedKey,
      isToday: key === today,
    };
  });
}

function parseDateTime(value: string) {
  const date = new Date(value || nowDateTimeLocal());
  const safe = Number.isFinite(date.getTime()) ? date : new Date();
  return {
    year: safe.getFullYear(),
    month: safe.getMonth() + 1,
    day: safe.getDate(),
    hour: safe.getHours(),
    minute: safe.getMinutes(),
    dateKey: formatDateKey(safe),
  };
}

function parseDateOnly(value: string) {
  const date = value ? new Date(`${value}T00:00`) : new Date();
  const safe = Number.isFinite(date.getTime()) ? date : new Date();
  return { year: safe.getFullYear(), month: safe.getMonth() + 1 };
}

function monthStart(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), 1);
}

function clampTime(event: Event, max: number) {
  const value = Number((event.target as HTMLInputElement).value);
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(max, Math.trunc(value)));
}

function formatDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}
