import type { Visibility, RowStatus } from "./types";

export const encoder = new TextEncoder();
export const decoder = new TextDecoder();
export const refreshCookieName = "memos_refresh";
export const accessCookieName = "memos_access";

export class HttpError extends Error {
  constructor(message: string, public status: number) {
    super(message);
  }
}

export function json(body: unknown, status = 200, cookies: string[] = []): Response {
  const headers = new Headers(corsHeaders());
  headers.set("Content-Type", "application/json; charset=utf-8");
  for (const value of cookies) headers.append("Set-Cookie", value);
  return new Response(JSON.stringify(body), { status, headers });
}

export function html(body: string): Response {
  return new Response(body, {
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store"
    }
  });
}

export function corsHeaders(): HeadersInit {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET,POST,PATCH,DELETE,OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type,Authorization"
  };
}

export async function readJson<T>(request: Request): Promise<T> {
  try {
    return await request.json<T>();
  } catch {
    throw new HttpError("Invalid JSON", 400);
  }
}

export function parseCookies(header: string | null): Map<string, string> {
  const cookies = new Map<string, string>();
  if (!header) return cookies;
  for (const part of header.split(";")) {
    const [name, ...rest] = part.trim().split("=");
    if (name) cookies.set(name, decodeURIComponent(rest.join("=")));
  }
  return cookies;
}

export function cookie(name: string, value: string, maxAge: number, httpOnly: boolean): string {
  const parts = [
    `${name}=${encodeURIComponent(value)}`,
    "Path=/api/v1",
    `Max-Age=${maxAge}`,
    "SameSite=Lax",
    "Secure"
  ];
  if (httpOnly) parts.push("HttpOnly");
  return parts.join("; ");
}

export function clearCookie(name: string): string {
  return `${name}=; Path=/api/v1; Max-Age=0; SameSite=Lax; Secure; HttpOnly`;
}

export function normalizeUsername(value: unknown): string {
  const username = String(value ?? "").trim().toLowerCase();
  if (!/^[a-z0-9_][a-z0-9_-]{2,31}$/.test(username)) {
    throw new HttpError("Username must be 3-32 lowercase letters, numbers, _ or -", 400);
  }
  return username;
}

export function assertPassword(value: unknown): asserts value is string {
  if (typeof value !== "string" || value.length < 8) {
    throw new HttpError("Password must be at least 8 characters", 400);
  }
}

export function normalizeVisibility(value: unknown, allowEmpty: boolean): Visibility | null {
  if ((value === null || value === undefined || value === "") && allowEmpty) return null;
  const visibility = String(value ?? "").toUpperCase();
  if (visibility === "PUBLIC" || visibility === "PROTECTED" || visibility === "PRIVATE") return visibility;
  if (allowEmpty) return null;
  throw new HttpError("Invalid visibility", 400);
}

export function normalizeState(value: unknown): RowStatus {
  const state = String(value ?? "NORMAL").toUpperCase();
  if (state === "NORMAL" || state === "ARCHIVED") return state;
  throw new HttpError("Invalid row status", 400);
}

export function stringOrEmpty(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function generateUid(prefix: string): string {
  return `${prefix}_${base64url(crypto.getRandomValues(new Uint8Array(12)))}`;
}

export function safeJsonParse<T>(value: string, fallback: T): T {
  try {
    return JSON.parse(value) as T;
  } catch {
    return fallback;
  }
}

export function sanitizeFilename(name: string): string {
  const cleaned = name.replace(/[\\/:*?"<>|\u0000-\u001f]/g, "_").trim();
  if (!cleaned || !/[A-Za-z0-9\p{L}\p{N}]/u.test(cleaned)) return "attachment";
  return cleaned.slice(0, 180);
}

export function unixNow(): number {
  return Math.floor(Date.now() / 1000);
}

export function clampNumber(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(Math.max(Math.floor(value), min), max);
}

export function encodePageToken(createdTs: number, id: number): string {
  return base64url(encoder.encode(JSON.stringify({ createdTs, id })));
}

export function decodePageToken(token: string | null): { createdTs: number; id: number } | null {
  if (!token) return null;
  try {
    const parsed = JSON.parse(decoder.decode(fromBase64url(token))) as { createdTs: number; id: number };
    if (Number.isFinite(parsed.createdTs) && Number.isFinite(parsed.id)) return parsed;
    return null;
  } catch {
    return null;
  }
}

export function base64url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

export function fromBase64url(value: string): Uint8Array {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(value.length / 4) * 4, "=");
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

export function constantTimeEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let index = 0; index < a.length; index += 1) diff |= a[index] ^ b[index];
  return diff === 0;
}

export function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}
