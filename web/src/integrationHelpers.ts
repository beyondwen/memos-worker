export function buildShareUrl(origin: string, shareUid: string): string {
  return `${origin.replace(/\/$/, "")}/#/shares/${encodeURIComponent(shareUid)}`;
}

export type WebhookFormResult =
  | { ok: true; name: string; url: string }
  | { ok: false; error: string };

export function normalizeWebhookForm(nameInput: string, urlInput: string): WebhookFormResult {
  const name = nameInput.trim();
  const url = urlInput.trim();
  if (!name) return { ok: false, error: "请输入 Webhook 名称" };
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      return { ok: false, error: "请输入有效的 Webhook URL" };
    }
  } catch {
    return { ok: false, error: "请输入有效的 Webhook URL" };
  }
  return { ok: true, name, url };
}
