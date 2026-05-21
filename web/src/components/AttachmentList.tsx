import { attachmentDisplayMeta } from "../attachmentView";
import type { Attachment } from "./MemoCard";

interface AttachmentListProps {
  attachments?: Attachment[];
}

export function AttachmentList({ attachments }: AttachmentListProps) {
  if (!attachments?.length) return null;

  return (
    <div class="memo-attachments">
      {attachments.map((att) => {
        const meta = attachmentDisplayMeta(att);
        return (
          <a
            key={att.uid}
            href={att.url}
            class={`memo-attachment${meta.isImage ? " image" : ""}`}
            target="_blank"
            rel="noopener noreferrer"
            title={`${att.filename} · ${meta.sizeLabel}`}
          >
            {meta.isImage && <img src={att.url} alt="" loading="lazy" />}
            <span class="attachment-icon">{meta.icon}</span>
            <span class="attachment-info">
              <span class="attachment-name">{att.filename}</span>
              <span class="attachment-meta">{meta.typeLabel} · {meta.sizeLabel}</span>
            </span>
          </a>
        );
      })}
    </div>
  );
}
