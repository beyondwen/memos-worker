import { buildApiUrl } from "./api";

export interface MemoSseEvent {
  type: string;
  name?: string;
}

const REFRESH_EVENT_TYPES = new Set([
  "memo.created",
  "memo.updated",
  "memo.archived",
  "memo.restored",
  "memo.deleted",
  "memo.bulk.updated",
  "memo.comment.created",
]);

export function shouldRefreshForSseEvent(event: unknown): event is MemoSseEvent {
  if (!event || typeof event !== "object") return false;
  const candidate = event as Partial<MemoSseEvent>;
  return typeof candidate.name === "string"
    && candidate.name.startsWith("memos/")
    && typeof candidate.type === "string"
    && REFRESH_EVENT_TYPES.has(candidate.type);
}

export function createMemoEventSource(): EventSource | null {
  if (typeof EventSource === "undefined") return null;
  return new EventSource(buildApiUrl("/api/v1/sse"), { withCredentials: true });
}
