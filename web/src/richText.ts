import { renderMarkdown } from "./markdown";

export function markdownToEditorHtml(markdown: string): string {
  return renderMarkdown(markdown || "");
}

export function editorHtmlToMarkdown(root: HTMLElement): string {
  return Array.from(root.childNodes)
    .map((node) => nodeToMarkdown(node).trimEnd())
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function nodeToMarkdown(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent ?? "";
  if (!(node instanceof HTMLElement)) return "";
  const tag = node.tagName.toLowerCase();
  if (tag === "br") return "\n";
  if (tag === "strong" || tag === "b") return `**${inlineChildren(node)}**`;
  if (tag === "em" || tag === "i") return `*${inlineChildren(node)}*`;
  if (tag === "code") return `\`${inlineChildren(node)}\``;
  if (tag === "a") {
    const text = inlineChildren(node);
    const href = node.getAttribute("href") || "";
    return href ? `[${text}](${href})` : text;
  }
  if (tag === "div" || tag === "p") return inlineChildren(node).trimEnd();
  if (tag === "h1") return `# ${inlineChildren(node)}`;
  if (tag === "h2") return `## ${inlineChildren(node)}`;
  if (tag === "h3") return `### ${inlineChildren(node)}`;
  if (tag === "blockquote") {
    return inlineChildren(node)
      .split(/\n+/)
      .map((line) => `> ${line}`)
      .join("\n");
  }
  if (tag === "pre") return `\`\`\`\n${node.textContent?.trimEnd() ?? ""}\n\`\`\``;
  if (tag === "ul" || tag === "ol") {
    return Array.from(node.children)
      .filter((child) => child.tagName.toLowerCase() === "li")
      .map((child, index) => `${tag === "ol" ? `${index + 1}.` : "-"} ${inlineChildren(child as HTMLElement)}`)
      .join("\n");
  }
  if (tag === "li") return `- ${inlineChildren(node)}`;
  return inlineChildren(node);
}

function inlineChildren(element: HTMLElement): string {
  return Array.from(element.childNodes)
    .map(nodeToMarkdown)
    .join("")
    .replace(/\u00a0/g, " ");
}

export function dateTimeLocalToUnix(value: string): number | null {
  if (!value) return null;
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return null;
  return Math.floor(date.getTime() / 1000);
}

export function unixToDateTimeLocal(ts: number): string {
  const date = new Date(ts * 1000);
  const offsetMs = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offsetMs).toISOString().slice(0, 16);
}

export function nowDateTimeLocal(): string {
  return unixToDateTimeLocal(Math.floor(Date.now() / 1000));
}

export function formatDateTimeLocalLabel(value: string): string {
  const date = new Date(value);
  if (!value || !Number.isFinite(date.getTime())) return "选择日期";
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");
  return `${year}/${month}/${day} ${hour}:${minute}`;
}

export function buildDateTimeLocal(year: number, month: number, day: number, hour: number, minute: number): string {
  const date = new Date(year, month - 1, day, hour, minute);
  return unixToDateTimeLocal(Math.floor(date.getTime() / 1000));
}

export function extractHashTags(content: string): string[] {
  const tags = new Set<string>();
  for (const match of content.matchAll(/#([\p{L}\p{N}_/-]+)/gu)) {
    const tag = match[1]?.slice(0, 64);
    if (tag) tags.add(tag);
  }
  return [...tags].sort((a, b) => a.localeCompare(b));
}
