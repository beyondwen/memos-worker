export function attachmentCleanupSummary(attachments: Array<{ size: number }>): {
  count: number;
  size: number;
  sizeLabel: string;
} {
  const size = attachments.reduce((sum, attachment) => sum + attachment.size, 0);
  return { count: attachments.length, size, sizeLabel: formatBytes(size) };
}

export function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}
