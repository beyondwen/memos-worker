import { useState, useRef } from "preact/hooks";
import { api } from "../api";

interface Attachment { uid: string; filename: string; }
interface MemoEditorProps { onCreated: (memo: unknown) => void; }

export function MemoEditor({ onCreated }: MemoEditorProps) {
  const [content, setContent] = useState("");
  const [visibility, setVisibility] = useState<"PRIVATE" | "PROTECTED" | "PUBLIC">("PRIVATE");
  const [attachmentUids, setAttachmentUids] = useState<string[]>([]);
  const [uploading, setUploading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [focused, setFocused] = useState(false);
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
    } catch (err) { alert(`上传失败：${(err as Error).message}`); }
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
      setContent(""); setAttachmentUids([]); onCreated(data.memo);
    } catch (err) { alert(`创建失败：${(err as Error).message}`); }
    finally { setSubmitting(false); }
  };

  const visLabel = { PRIVATE: "私有", PROTECTED: "登录可见", PUBLIC: "公开" };

  return (
    <div class="editor-card">
      <textarea
        class="editor-textarea"
        placeholder="记录点什么..."
        value={content}
        onInput={(e) => setContent((e.target as HTMLTextAreaElement).value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setTimeout(() => setFocused(false), 150)}
        onKeyDown={(e) => { if ((e.metaKey || e.ctrlKey) && e.key === "Enter") submit(); }}
      />

      {(focused || content.trim()) && (
        <div class="editor-actions">
          <select
            value={visibility}
            onChange={(e) => setVisibility((e.target as HTMLSelectElement).value as "PRIVATE" | "PROTECTED" | "PUBLIC")}
          >
            <option value="PRIVATE">私有</option>
            <option value="PROTECTED">登录可见</option>
            <option value="PUBLIC">公开</option>
          </select>

          <input ref={fileRef} type="file" multiple style={{ display: "none" }} onChange={handleFile} />
          <button class="btn btn-ghost btn-sm" onClick={() => fileRef.current?.click()} disabled={uploading}>
            {uploading ? "上传中..." : "📎 附件"}
          </button>

          {attachmentUids.length > 0 && (
            <span style={{ fontSize: "0.75rem", color: "var(--zinc-400)" }}>
              {attachmentUids.length} 个附件
              <button class="btn-ghost btn-sm" onClick={() => setAttachmentUids([])} style={{ padding: "0 4px", marginLeft: 4 }}>
                &times;
              </button>
            </span>
          )}

          <div class="spacer" />

          <button class="btn btn-primary btn-sm" onClick={submit} disabled={submitting || !content.trim()}>
            {submitting ? "发布中..." : "发布"}
          </button>
        </div>
      )}
    </div>
  );
}
