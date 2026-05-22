type Notify = (message: string, kind?: "success" | "error" | "info") => void;

export function reportSettingsLoadError(
  section: string,
  err: unknown,
  notify?: Notify
) {
  const message = err instanceof Error ? err.message : String(err);
  console.warn(`[settings] ${section} load failed:`, err);
  notify?.(`${section}加载失败：${message}`, "error");
}
