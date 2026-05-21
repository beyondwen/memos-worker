import { useState, useRef } from "preact/hooks";
import { api } from "../api";
import { useFeedback } from "./Feedback";

interface Attachment { uid: string; filename: string; }
interface MemoEditorProps { onCreated: (memo: unknown) => void; }

export function MemoEditor({ onCreated }: MemoEditorProps) {
  const { notify } = useFeedback();
  const [content, setContent] = useState("");
  const [visibility, setVisibility] = useState<"PRIVATE" | "PROTECTED" | "PUBLIC">("PRIVATE");
  const [attachmentUids, setAttachmentUids] = useState<string[]>([]);
  const [uploading, setUploading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const handleFile = async (e: Event) => {
    const input = e.target as HTMLInputElement;
    if (!input.files?.length) return;
    setUploading(true);
    try {
      for (const file of Array.from(input.files)) {
        const form = new FormData();
        form.append("file", file);
        const data = await api<{ attachment: Attachment }>("/api/v1/attachments", { method: "POST", body: form });
        if (data.attachment) setAttachmentUids((p) => [...p, data.attachment.uid]);
      }
    } catch (err) { notify(`上传失败：${(err as Error).message}`, "error"); }
    finally { setUploading(false); if (fileRef.current) fileRef.current.value = ""; }
  };

  const submit = async () => {
    const trimmed = content.trim();
    if (!trimmed) return;
    setSubmitting(true);
    try {
      const data = await api<{ memo: unknown }>("/api/v1/memos", {
        method: "POST",
        body: JSON.stringify({ content: trimmed, visibility, attachmentUids }),
      });
      setContent(""); setAttachmentUids([]); onCreated(data.memo); notify("备忘录已发布", "success");
    } catch (err) { notify(`创建失败：${(err as Error).message}`, "error"); }
    finally { setSubmitting(false); }
  };

  const visLabel = { PRIVATE: "私有", PROTECTED: "登录可见", PUBLIC: "公开" };

  return (
    <div class="editor-card">
      <div class="editor-header">
        <div>
          <div class="editor-kicker">Quick Memo</div>
          <div class="editor-title">记录一点想法</div>
        </div>
      </div>

      <textarea
        class="editor-textarea"
        placeholder="记录点什么..."
        value={content}
        onInput={(e) => setContent((e.target as HTMLTextAreaElement).value)}
        onKeyDown={(e) => { if ((e.metaKey || e.ctrlKey) && e.key === "Enter") submit(); }}
      />

      <div class="editor-actions">
        <div class="visibility-segment" aria-label="可见性">
          {(Object.keys(visLabel) as Array<keyof typeof visLabel>).map((key) => (
            <button
              key={key}
              type="button"
              class={visibility === key ? "active" : ""}
              onClick={() => setVisibility(key)}
            >
              {visLabel[key]}
            </button>
          ))}
        </div>

        <input ref={fileRef} type="file" multiple style={{ display: "none" }} onChange={handleFile} />
        <button class="btn btn-ghost btn-sm tool-button" onClick={() => fileRef.current?.click()} disabled={uploading}>
          <span aria-hidden="true">+</span>
          {uploading ? "上传中" : "附件"}
        </button>

        {attachmentUids.length > 0 && (
          <span class="attachment-count">
            {attachmentUids.length} 个附件
            <button class="inline-clear" onClick={() => setAttachmentUids([])} aria-label="清空附件">
              ×
            </button>
          </span>
        )}

        <div class="spacer" />

        <button class="btn btn-primary btn-sm" onClick={submit} disabled={submitting || !content.trim()}>
          {submitting ? "发布中..." : "发布"}
        </button>
      </div>
    </div>
  );
}
