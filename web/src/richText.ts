import { renderMarkdown } from "./markdown";

export function markdownToEditorHtml(markdown: string): string {
  return renderMarkdown(markdown || "");
}

export function editorHtmlToMarkdown(root: HTMLElement): string {
  const blocks: string[] = [];
  root.childNodes.forEach((node) => {
    const block = nodeToMarkdown(node).trimEnd();
    if (block) blocks.push(block);
  });
  return blocks.join("\n\n").trim();
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
    .replace(/\u00a0/g, " ")
    .trim();
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
