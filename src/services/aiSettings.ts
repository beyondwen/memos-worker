import type { Env, Viewer } from "../types";
import { HttpError, json, readJson, safeJsonParse } from "../utils";

const AI_SETTINGS_KEY = "ai.settings";
const DEFAULT_AI_BASE_URL = "https://api.openai.com/v1";
const DEFAULT_AI_MODEL = "gpt-4o-mini";

export interface AiSettings {
  baseUrl: string;
  model: string;
  apiKey: string;
}

export interface AiSettingsClient {
  baseUrl: string;
  model: string;
  configured: boolean;
}

export function sanitizeAiSettingsForClient(settings: AiSettings): AiSettingsClient {
  return {
    baseUrl: settings.baseUrl,
    model: settings.model,
    configured: Boolean(settings.apiKey.trim()),
  };
}

export function mergeAiSettingsUpdate(previous: AiSettings, update: Partial<AiSettings>): AiSettings {
  return {
    baseUrl: normalizeBaseUrl(update.baseUrl ?? previous.baseUrl),
    model: String(update.model ?? previous.model).trim() || previous.model || DEFAULT_AI_MODEL,
    apiKey: update.apiKey === undefined || !String(update.apiKey).trim()
      ? previous.apiKey
      : String(update.apiKey).trim(),
  };
}

export async function resolveAiRuntimeSettings(env: Env): Promise<AiSettings> {
  const stored = await readStoredAiSettings(env);
  return {
    baseUrl: normalizeBaseUrl(stored.baseUrl || env.AI_BASE_URL || DEFAULT_AI_BASE_URL),
    model: stored.model || env.AI_MODEL || DEFAULT_AI_MODEL,
    apiKey: stored.apiKey || env.AI_API_KEY || "",
  };
}

export async function getAiSettings(env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  return json({ settings: sanitizeAiSettingsForClient(await resolveAiRuntimeSettings(env)) });
}

export async function updateAiSettings(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const body = await readJson<Partial<AiSettings>>(request);
  const previous = await readStoredAiSettings(env);
  const next = mergeAiSettingsUpdate(previous, body);
  await writeStoredAiSettings(env, next);
  return json({ settings: sanitizeAiSettingsForClient(next) });
}

export async function testAiSettings(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const body = await readJson<Partial<AiSettings>>(request);
  const base = await resolveAiRuntimeSettings(env);
  const settings = mergeAiSettingsUpdate(base, body);
  if (!settings.apiKey) return json({ error: "AI API Key is required" }, 400);

  const response = await fetch(`${settings.baseUrl.replace(/\/+$/, "")}/chat/completions`, {
    method: "POST",
    headers: {
      "Authorization": `Bearer ${settings.apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      model: settings.model,
      temperature: 0,
      messages: [
        { role: "system", content: "Return ok." },
        { role: "user", content: "ping" },
      ],
      max_tokens: 8,
    }),
  });
  if (!response.ok) throw new HttpError(`AI API returned HTTP ${response.status}`, 502);
  return json({ ok: true });
}

async function readStoredAiSettings(env: Env): Promise<AiSettings> {
  const row = await env.DB.prepare("SELECT value FROM system_setting WHERE name = ?")
    .bind(AI_SETTINGS_KEY)
    .first<{ value: string }>();
  const stored = safeJsonParse<Partial<AiSettings>>(row?.value ?? "{}", {});
  return {
    baseUrl: normalizeBaseUrl(stored.baseUrl || DEFAULT_AI_BASE_URL),
    model: String(stored.model || DEFAULT_AI_MODEL).trim() || DEFAULT_AI_MODEL,
    apiKey: String(stored.apiKey || "").trim(),
  };
}

async function writeStoredAiSettings(env: Env, settings: AiSettings): Promise<void> {
  await env.DB.prepare(`
    INSERT INTO system_setting (name, value, description)
    VALUES (?, ?, ?)
    ON CONFLICT(name) DO UPDATE SET value = excluded.value
  `).bind(AI_SETTINGS_KEY, JSON.stringify(settings), "AI model settings").run();
}

function normalizeBaseUrl(value: string): string {
  const raw = String(value || DEFAULT_AI_BASE_URL).trim() || DEFAULT_AI_BASE_URL;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    throw new HttpError("Invalid AI Base URL", 400);
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new HttpError("Invalid AI Base URL", 400);
  }
  url.hash = "";
  url.search = "";
  return url.toString().replace(/\/+$/, "");
}
