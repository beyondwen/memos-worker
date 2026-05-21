import type { Env, Viewer, DbUser } from "./types";
import { verifyJwt } from "./auth";
import { sha256Hex } from "./auth";
import { parseCookies, accessCookieName, unixNow } from "./utils";

export async function getUserById(env: Env, id: number): Promise<DbUser | null> {
  return await env.DB.prepare('SELECT * FROM "user" WHERE id = ?').bind(id).first<DbUser>();
}

export async function currentViewer(request: Request, env: Env): Promise<Viewer | null> {
  const auth = request.headers.get("Authorization");
  let claims: Awaited<ReturnType<typeof verifyJwt>> = null;

  if (auth?.startsWith("Bearer ")) {
    const token = auth.slice("Bearer ".length).trim();
    if (token.startsWith("memos_pat_")) return viewerFromPat(env, token);
    claims = await verifyJwt(token, env.SERVER_SECRET);
  }

  if (!claims) {
    const cookies = parseCookies(request.headers.get("Cookie"));
    const accessToken = cookies.get(accessCookieName);
    if (accessToken) claims = await verifyJwt(accessToken, env.SERVER_SECRET);
  }

  if (!claims) {
    const token = new URL(request.url).searchParams.get("access_token");
    if (token) claims = await verifyJwt(token, env.SERVER_SECRET);
  }

  if (!claims || claims.type !== "access") return null;
  const user = await getUserById(env, Number(claims.sub));
  if (!user || user.row_status !== "NORMAL") return null;
  return { id: user.id, username: user.username, role: user.role, rowStatus: user.row_status };
}

export async function viewerFromPat(env: Env, token: string): Promise<Viewer | null> {
  const prefix = token.slice(0, 20);
  const tokenHash = await sha256Hex(token);
  const row = await env.DB.prepare(`
    SELECT user_access_token.user_id
    FROM user_access_token
    JOIN "user" ON "user".id = user_access_token.user_id
    WHERE user_access_token.token_prefix = ?
      AND user_access_token.token_hash = ?
      AND user_access_token.row_status = 'NORMAL'
      AND (user_access_token.expires_ts IS NULL OR user_access_token.expires_ts > ?)
      AND "user".row_status = 'NORMAL'
  `).bind(prefix, tokenHash, unixNow()).first<{ user_id: number }>();
  if (!row) return null;
  await env.DB.prepare("UPDATE user_access_token SET last_used_ts = ? WHERE token_hash = ?")
    .bind(unixNow(), tokenHash)
    .run();
  const user = await getUserById(env, row.user_id);
  return user ? { id: user.id, username: user.username, role: user.role, rowStatus: user.row_status } : null;
}
