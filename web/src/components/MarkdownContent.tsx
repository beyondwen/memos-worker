import { useMemo } from "preact/hooks";
import { renderMarkdown } from "../markdown";
import { highlightRenderedHtml } from "../searchHighlight";

interface MarkdownContentProps {
  content: string;
  highlight?: string;
}

export function MarkdownContent({ content, highlight = "" }: MarkdownContentProps) {
  const html = useMemo(() => {
    const rendered = renderMarkdown(content);
    return highlightRenderedHtml(rendered, highlight);
  }, [content, highlight]);

  return (
    <div
      class="memo-content"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
