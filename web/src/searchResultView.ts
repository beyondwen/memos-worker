export function scoreSearchMatch(item: { content: string; tags?: string[] }, query: string): number {
  const term = query.trim().toLowerCase();
  if (!term) return 0;
  let score = 0;
  if (item.tags?.some((tag) => tag.toLowerCase() === term)) score += 50;
  if (item.tags?.some((tag) => tag.toLowerCase().includes(term))) score += 20;
  const content = item.content.toLowerCase();
  if (content.startsWith(term)) score += 15;
  if (content.includes(term)) score += 10;
  return score;
}

export function buildSearchSnippet(content: string, query: string, radius = 24): string {
  const term = query.trim();
  if (!term) return content.slice(0, radius * 2);
  const index = content.toLowerCase().indexOf(term.toLowerCase());
  if (index < 0) return content.slice(0, radius * 2);
  const side = Math.max(1, Math.floor(radius / 2));
  const start = Math.max(0, index - side);
  const end = Math.min(content.length, index + term.length + side);
  return `${start > 0 ? "..." : ""}${content.slice(start, end)}${end < content.length ? "..." : ""}`;
}
