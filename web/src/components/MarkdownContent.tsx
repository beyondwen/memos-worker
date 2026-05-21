import { useMemo } from "preact/hooks";
import { renderMarkdown } from "../markdown";

interface MarkdownContentProps {
  content: string;
}

export function MarkdownContent({ content }: MarkdownContentProps) {
  const html = useMemo(() => renderMarkdown(content), [content]);

  return (
    <div
      class="memo-content"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
