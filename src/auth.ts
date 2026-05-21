import type { Claims } from "./types";
import { encoder, base64url, fromBase64url, constantTimeEqual, toArrayBuffer, unixNow } from "./utils";

export async function hashPassword(password: string, iterations = 100000): Promise<string> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const key = await crypto.subtle.importKey("raw", encoder.encode(password), "PBKDF2", false, ["deriveBits"]);
  const bits = await crypto.subtle.deriveBits({
    name: "PBKDF2",
    hash: "SHA-256",
    salt,
    iterations
  }, key, 256);
  return `pbkdf2_sha256$${iterations}$${base64url(salt)}$${base64url(new Uint8Array(bits))}`;
}

export async function verifyPassword(password: string, stored: string): Promise<boolean> {
  const [algorithm, iterationText, saltText, hashText] = stored.split("$");
  if (algorithm !== "pbkdf2_sha256") return false;
  const iterations = Number(iterationText);
  if (!Number.isInteger(iterations) || iterations < 1) return false;

  const salt = fromBase64url(saltText);
  const expected = fromBase64url(hashText);
  const key = await crypto.subtle.importKey("raw", encoder.encode(password), "PBKDF2", false, ["deriveBits"]);
  const bits = await crypto.subtle.deriveBits({
    name: "PBKDF2",
    hash: "SHA-256",
    salt: toArrayBuffer(salt),
    iterations
  }, key, expected.length * 8);
  return constantTimeEqual(new Uint8Array(bits), expected);
}

export async function signJwt(claims: Claims, secret: string): Promise<string> {
  const header = { alg: "HS256", typ: "JWT" };
  const payload = base64url(encoder.encode(JSON.stringify(claims)));
  const protectedHeader = base64url(encoder.encode(JSON.stringify(header)));
  const data = `${protectedHeader}.${payload}`;
  const signature = await hmacSha256(data, secret);
  return `${data}.${base64url(signature)}`;
}

export async function verifyJwt(token: string, secret: string): Promise<Claims | null> {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  const [header, payload, signature] = parts;
  const expected = await hmacSha256(`${header}.${payload}`, secret);
  if (!constantTimeEqual(fromBase64url(signature), expected)) return null;

  const claims = JSON.parse(new TextDecoder().decode(fromBase64url(payload))) as Claims;
  if (!claims.exp || claims.exp < unixNow()) return null;
  if (claims.iss !== "memos-worker") return null;
  return claims;
}

async function hmacSha256(data: string, secret: string): Promise<Uint8Array> {
  const key = await crypto.subtle.importKey("raw", encoder.encode(secret), {
    name: "HMAC",
    hash: "SHA-256"
  }, false, ["sign"]);
  return new Uint8Array(await crypto.subtle.sign("HMAC", key, encoder.encode(data)));
}

export async function sha256Hex(value: string): Promise<string> {
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", encoder.encode(value)));
  return [...hash].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
