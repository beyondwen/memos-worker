export type MemoVisibility = "PRIVATE" | "PROTECTED" | "PUBLIC";
export type MemoState = "NORMAL" | "ARCHIVED";
export type MemoPropertyFilter = "has_task_list" | "has_link" | "has_code" | "has_incomplete_tasks";

interface MemoListPathOptions {
  basePath?: string;
  tag?: string;
  visibility?: MemoVisibility;
  state?: MemoState;
  search?: string;
  propertyFilter?: MemoPropertyFilter;
  advancedFilter?: string;
  createdAfter?: string;
  createdBefore?: string;
  pageToken?: string;
  pageSize?: number;
}

function quoteFilterString(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

export function buildMemoFilter(search?: string, propertyFilter?: MemoPropertyFilter, advancedFilter?: string): string {
  const parts: string[] = [];
  const trimmed = search?.trim();
  if (trimmed) parts.push(`content.contains(${quoteFilterString(trimmed)})`);
  if (propertyFilter) parts.push(propertyFilter);
  const advanced = advancedFilter?.trim();
  if (advanced) parts.push(`(${advanced})`);
  return parts.join(" && ");
}

export function buildMemoListPath(options: MemoListPathOptions = {}): string {
  const url = new URL(options.basePath ?? "/api/v1/memos", "https://memos.local");
  const filter = buildMemoFilter(options.search, options.propertyFilter, options.advancedFilter);

  if (options.tag) url.searchParams.set("tag", options.tag);
  if (options.visibility) url.searchParams.set("visibility", options.visibility);
  if (options.state) url.searchParams.set("state", options.state);
  if (filter) url.searchParams.set("filter", filter);
  if (options.createdAfter) url.searchParams.set("created_after", options.createdAfter);
  if (options.createdBefore) url.searchParams.set("created_before", options.createdBefore);
  if (options.pageToken) url.searchParams.set("page_token", options.pageToken);
  url.searchParams.set("page_size", String(options.pageSize ?? 20));

  return url.pathname + url.search;
}
