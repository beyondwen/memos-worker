import type { MemoVisibility } from "./memoQuery";

export type BulkMemoAction = "ARCHIVE" | "RESTORE" | "DELETE" | "VISIBILITY";

export interface BulkMemoBody {
  action: BulkMemoAction;
  memoUids: string[];
  visibility?: MemoVisibility;
}

export function buildBulkMemoRequest(
  action: BulkMemoAction,
  memoUids: string[],
  visibility?: MemoVisibility
): { ok: true; body: BulkMemoBody } | { ok: false; error: string } {
  const uniqueUids = [...new Set(memoUids.map((uid) => uid.trim()).filter(Boolean))];
  if (uniqueUids.length === 0) return { ok: false, error: "请选择备忘录" };
  if (action === "VISIBILITY" && !visibility) return { ok: false, error: "请选择新的可见性" };

  return {
    ok: true,
    body: {
      action,
      memoUids: uniqueUids,
      ...(action === "VISIBILITY" ? { visibility } : {}),
    },
  };
}

export function bulkMemoActionLabel(action: BulkMemoAction): string {
  return {
    ARCHIVE: "归档",
    RESTORE: "恢复",
    DELETE: "彻底删除",
    VISIBILITY: "修改可见性",
  }[action];
}
