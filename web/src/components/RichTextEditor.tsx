import { useEffect, useRef } from "preact/hooks";
import { editorHtmlToMarkdown, markdownToEditorHtml } from "../richText";

interface RichTextEditorProps {
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
  onSubmit?: () => void;
}

export function RichTextEditor({ value, placeholder = "", onChange, onSubmit }: RichTextEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const selfChangeRef = useRef(false);

  useEffect(() => {
    if (selfChangeRef.current) {
      selfChangeRef.current = false;
      return;
    }
    const editor = editorRef.current;
    if (editor) editor.innerHTML = markdownToEditorHtml(value);
  }, [value]);

  const sync = () => {
    const editor = editorRef.current;
    if (!editor) return;
    selfChangeRef.current = true;
    onChange(editorHtmlToMarkdown(editor));
  };

  const command = (name: string, valueArg?: string) => {
    editorRef.current?.focus();
    document.execCommand(name, false, valueArg);
    sync();
  };

  const link = () => {
    const href = window.prompt("链接地址");
    if (!href?.trim()) return;
    command("createLink", href.trim());
  };

  return (
    <div class="rich-editor">
      <div class="rich-toolbar" aria-label="富文本工具栏">
        <button type="button" title="加粗" onClick={() => command("bold")}>B</button>
        <button type="button" title="斜体" onClick={() => command("italic")}>I</button>
        <button type="button" title="标题" onClick={() => command("formatBlock", "h2")}>H</button>
        <button type="button" title="引用" onClick={() => command("formatBlock", "blockquote")}>“</button>
        <button type="button" title="无序列表" onClick={() => command("insertUnorderedList")}>•</button>
        <button type="button" title="有序列表" onClick={() => command("insertOrderedList")}>1.</button>
        <button type="button" title="代码" onClick={() => command("formatBlock", "pre")}>{"</>"}</button>
        <button type="button" title="链接" onClick={link}>↗</button>
      </div>
      <div
        ref={editorRef}
        class="rich-editor-surface"
        contentEditable
        data-placeholder={placeholder}
        onInput={sync}
        onKeyDown={(event) => {
          if ((event.metaKey || event.ctrlKey) && event.key === "Enter") onSubmit?.();
        }}
      />
    </div>
  );
}
