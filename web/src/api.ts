let accessToken = localStorage.getItem("memos_access") || "";
let refreshPromise: Promise<boolean> | null = null;

export function getToken(): string {
  return accessToken;
}

export function setToken(token: string): void {
  accessToken = token;
  localStorage.setItem("memos_access", token);
}

export function clearToken(): void {
  accessToken = "";
  localStorage.removeItem("memos_access");
}

export async function api<T = unknown>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const headers = new Headers(options.headers);
  if (!(options.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  if (accessToken) {
    headers.set("Authorization", "Bearer " + accessToken);
  }

  let response = await fetch(path, { ...options, headers });

  if (response.status === 401 && accessToken) {
    const refreshed = await tryRefresh();
    if (refreshed) {
      headers.set("Authorization", "Bearer " + accessToken);
      response = await fetch(path, { ...options, headers });
    }
  }

  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    throw new Error(body.error || `HTTP ${response.status}`);
  }

  if (response.headers.get("Content-Type")?.includes("application/json")) {
    return (await response.json()) as T;
  }
  return undefined as T;
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
    const res = await fetch("/api/v1/auth/refresh", {
      method: "POST",
      credentials: "include",
    });
    if (!res.ok) return false;
    const data = await res.json();
    if (data.accessToken) {
      setToken(data.accessToken);
      return true;
    }
    return false;
  } catch {
    return false;
  }
}
