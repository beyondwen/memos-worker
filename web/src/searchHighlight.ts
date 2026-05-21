export function highlightRenderedHtml(html: string, term: string): string {
  const query = term.trim();
  if (!query) return html;
  const regex = new RegExp(`(${escapeRegExp(query)})`, "gi");
  return html
    .split(/(<[^>]+>)/g)
    .map((part) => {
      if (!part || part.startsWith("<")) return part;
      return part.replace(regex, '<mark class="search-hit">$1</mark>');
    })
    .join("");
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
