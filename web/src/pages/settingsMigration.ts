import { apiFetch } from "../api";
import { parseMigrationStreamEvent, type MigrationProgress } from "./settingsModel";

export async function runMigrationStream(
  path: string,
  payload: {
    baseUrl: string;
    accessToken: string;
    includeArchived: boolean;
  },
  onProgress: (progress: MigrationProgress) => void
): Promise<MigrationProgress> {
  const response = await apiFetch(path, {
    method: "POST",
    body: JSON.stringify(payload),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    throw new Error(body.error || `HTTP ${response.status}`);
  }
  if (!response.body) throw new Error("浏览器不支持读取迁移进度");

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let finalResult: MigrationProgress | null = null;

  while (true) {
    const { value, done } = await reader.read();
    buffer += decoder.decode(value ?? new Uint8Array(), { stream: !done });
    let boundary = buffer.indexOf("\n\n");
    while (boundary >= 0) {
      const rawEvent = buffer.slice(0, boundary);
      buffer = buffer.slice(boundary + 2);
      const event = parseMigrationStreamEvent(rawEvent);
      if (event.name === "error") {
        throw new Error(String((event.data as { error?: string }).error || "迁移失败"));
      }
      if (event.name === "progress" || event.name === "done") {
        const progress = event.data as MigrationProgress;
        onProgress(progress);
        if (event.name === "done") finalResult = progress;
      }
      boundary = buffer.indexOf("\n\n");
    }
    if (done) break;
  }

  if (!finalResult) throw new Error("迁移进度流提前结束");
  return finalResult;
}
