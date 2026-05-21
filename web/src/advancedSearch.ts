export type PinnedFilter = "" | "PINNED" | "UNPINNED";

export interface AdvancedSearchState {
  creator?: string;
  pinned?: PinnedFilter;
  createdAfter?: string;
  createdBefore?: string;
}

function quoteFilterString(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

function unixDateStart(date: string): number | null {
  if (!date) return null;
  const ms = Date.parse(`${date}T00:00:00Z`);
  return Number.isFinite(ms) ? Math.floor(ms / 1000) : null;
}

export function buildAdvancedMemoFilter(state: AdvancedSearchState): string {
  const parts: string[] = [];
  const creator = state.creator?.trim();
  if (creator) parts.push(`creator == ${quoteFilterString(creator)}`);
  if (state.pinned === "PINNED") parts.push("pinned == true");
  if (state.pinned === "UNPINNED") parts.push("pinned == false");
  const after = unixDateStart(state.createdAfter ?? "");
  if (after !== null) parts.push(`created_ts >= ${after}`);
  const before = unixDateStart(state.createdBefore ?? "");
  if (before !== null) parts.push(`created_ts <= ${before}`);
  return parts.join(" && ");
}
