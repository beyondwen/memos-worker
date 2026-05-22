type ViteImportMeta = ImportMeta & {
  env?: {
    VITE_API_BASE_URL?: string;
  };
};

const API_BASE_URL = ((import.meta as ViteImportMeta).env?.VITE_API_BASE_URL || "").trim();
const CSRF_COOKIE = "memos_csrf";
const CSRF_HEADER = "X-CSRF-Token";

function getBrowserStorage(): Storage | null {
  return typeof window === "undefined" ? null : window.localStorage;
}

let accessToken = "";
let refreshPromise: Promise<boolean> | null = null;
let authExpiredHandler: (() => void) | null = null;

export function getToken(): string {
  return accessToken;
}

export function setToken(token: string): void {
  accessToken = token;
}

export function clearToken(): void {
  accessToken = "";
  getBrowserStorage()?.removeItem("memos_access");
}

export function setAuthExpiredHandler(handler: (() => void) | null): void {
  authExpiredHandler = handler;
}

export async function api<T = unknown>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const response = await apiFetch(path, options);

  if (!response.ok) {
    const body = await response.json().catch(() => ({})) as { error?: string };
    throw new Error(body.error || `HTTP ${response.status}`);
  }

  if (response.headers.get("Content-Type")?.includes("application/json")) {
    return (await response.json()) as T;
  }
  return undefined as T;
}

export async function apiFetch(
  path: string,
  options: RequestInit = {}
): Promise<Response> {
  const headers = new Headers(options.headers);
  if (!(options.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  if (accessToken) {
    headers.set("Authorization", "Bearer " + accessToken);
  }
  applyCsrfHeader(headers, options.method);

  let response = await fetch(buildApiUrl(path), { credentials: "include", ...options, headers });

  if (response.status === 401 && shouldAttemptRefresh(path)) {
    const hadAccessToken = Boolean(accessToken);
    const refreshed = await tryRefresh();
    if (refreshed) {
      headers.set("Authorization", "Bearer " + accessToken);
      applyCsrfHeader(headers, options.method);
      response = await fetch(buildApiUrl(path), { credentials: "include", ...options, headers });
    } else {
      clearToken();
      if (hadAccessToken) authExpiredHandler?.();
    }
  }

  return response;
}

function applyCsrfHeader(headers: Headers, method: string | undefined): void {
  if (isSafeMethod(method) || headers.has(CSRF_HEADER)) return;
  const token = readCookie(CSRF_COOKIE);
  if (token) headers.set(CSRF_HEADER, token);
}

function isSafeMethod(method: string | undefined): boolean {
  return ["GET", "HEAD", "OPTIONS"].includes((method || "GET").toUpperCase());
}

function readCookie(name: string): string {
  if (typeof document === "undefined") return "";
  const prefix = `${name}=`;
  return document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(prefix))
    ?.slice(prefix.length) || "";
}

async function tryRefresh(): Promise<boolean> {
  if (refreshPromise) return refreshPromise;
  refreshPromise = doRefresh();
  const result = await refreshPromise;
  refreshPromise = null;
  return result;
}

async function doRefresh(): Promise<boolean> {
  try {
    const res = await fetch(buildApiUrl("/api/v1/auth/refresh"), {
      method: "POST",
      credentials: "include",
    });
    if (!res.ok) return false;
    const data = await res.json() as { accessToken?: string };
    if (data.accessToken) {
      setToken(data.accessToken);
      return true;
    }
    return false;
  } catch (err) {
    console.warn("[api] token refresh failed:", err);
    return false;
  }
}

export function buildApiUrl(path: string, baseUrl: string = API_BASE_URL): string {
  if (!baseUrl.trim() || /^https?:\/\//i.test(path)) return path;
  return `${baseUrl.replace(/\/+$/, "")}/${path.replace(/^\/+/, "")}`;
}

function shouldAttemptRefresh(path: string): boolean {
  const url = /^https?:\/\//i.test(path) ? new URL(path) : new URL(path, "https://memos.local");
  return ![
    "/api/v1/instance",
    "/api/v1/setup",
    "/api/v1/auth/signin",
    "/api/v1/auth/signup",
    "/api/v1/auth/refresh",
    "/api/v1/auth/signout",
  ].includes(url.pathname);
}
