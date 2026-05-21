export interface HomeDateFilters {
  createdAfter: string;
  createdBefore: string;
}

const HOME_DATE_FILTER_KEYS = ["createdAfter", "createdBefore"];

export function parseHomeDateFilterParams(search: string): HomeDateFilters {
  const params = new URLSearchParams(search);
  return {
    createdAfter: params.get("createdAfter") ?? "",
    createdBefore: params.get("createdBefore") ?? "",
  };
}

export function buildHomeDateFilterPath(day: string): string {
  const params = new URLSearchParams();
  params.set("createdAfter", day);
  params.set("createdBefore", day);
  return `/?${params.toString()}`;
}

export function stripHomeFilterParams(path: string): string {
  const url = new URL(path, "https://memos.local");
  for (const key of HOME_DATE_FILTER_KEYS) {
    url.searchParams.delete(key);
  }
  const query = url.searchParams.toString();
  return `${url.pathname}${query ? `?${query}` : ""}`;
}
