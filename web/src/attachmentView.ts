interface AttachmentLike {
  filename: string;
  type?: string;
  size?: number;
}

interface AttachmentDisplayMeta {
  icon: string;
  isImage: boolean;
  sizeLabel: string;
  typeLabel: string;
}

const TYPE_LABELS: Record<string, string> = {
  "application/pdf": "PDF",
  "text/plain": "TXT",
  "text/markdown": "MD",
  "application/zip": "ZIP",
  "application/json": "JSON",
};

function extensionOf(filename: string): string {
  const ext = filename.split(".").pop();
  if (!ext || ext === filename) return "FILE";
  return ext.slice(0, 4).toUpperCase();
}

export function formatAttachmentSize(size = 0): string {
  if (!Number.isFinite(size) || size <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = size;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const rounded = value >= 10 || unit === 0 ? Math.round(value) : Math.round(value * 10) / 10;
  return `${rounded} ${units[unit]}`;
}

export function attachmentDisplayMeta(attachment: AttachmentLike): AttachmentDisplayMeta {
  const type = attachment.type || "application/octet-stream";
  const isImage = type.startsWith("image/");
  const fallback = extensionOf(attachment.filename);
  const typeLabel = isImage
    ? type.replace("image/", "").slice(0, 4).toUpperCase() || fallback
    : TYPE_LABELS[type] ?? fallback;

  return {
    icon: isImage ? "IMG" : typeLabel,
    isImage,
    sizeLabel: formatAttachmentSize(attachment.size),
    typeLabel,
  };
}
