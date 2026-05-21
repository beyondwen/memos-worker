export interface MemoSseEvent {
  type: string;
  name?: string;
}

const REFRESH_EVENT_TYPES = new Set([
  "memo.created",
  "memo.updated",
  "memo.deleted",
  "memo.comment.created",
  "reaction.upserted",
  "reaction.deleted",
]);

export function shouldRefreshForSseEvent(event: unknown): event is MemoSseEvent {
  if (!event || typeof event !== "object") return false;
  const candidate = event as Partial<MemoSseEvent>;
  return typeof candidate.name === "string"
    && candidate.name.startsWith("memos/")
    && typeof candidate.type === "string"
    && REFRESH_EVENT_TYPES.has(candidate.type);
}

export function createMemoEventSource(token: string): EventSource | null {
  if (!token || typeof EventSource === "undefined") return null;
  return new EventSource(`/api/v1/sse?access_token=${encodeURIComponent(token)}`);
}
