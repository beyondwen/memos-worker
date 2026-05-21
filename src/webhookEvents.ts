export const MEMO_WEBHOOK_EVENTS = [
  "memo.created",
  "memo.updated",
  "memo.archived",
  "memo.restored",
  "memo.deleted",
  "memo.bulk.updated",
  "memo.comment.created",
  "reaction.upserted",
  "reaction.deleted",
  "share.created",
  "share.deleted",
] as const;

export type MemoWebhookEvent = typeof MEMO_WEBHOOK_EVENTS[number];
