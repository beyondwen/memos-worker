export function buildAuthRedirectPath(currentPath: string): string {
  const path = currentPath || "/";
  if (path === "/auth" || path.startsWith("/auth?")) return "/auth";
  return `/auth?redirect=${encodeURIComponent(path)}`;
}

export function currentRoutePath(): string {
  if (typeof window === "undefined") return "/";
  const hashPath = window.location.hash.startsWith("#/")
    ? window.location.hash.slice(1)
    : "";
  return hashPath || window.location.pathname + window.location.search;
}
