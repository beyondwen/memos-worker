import type { Env, Viewer, DbUser, DbUserAccessToken, DbUserSetting } from "../types";
import {
  json, readJson, unixNow, generateUid, HttpError, stringOrEmpty, normalizeUsername, assertPassword,
  clearCookie, refreshCookieName, accessCookieName
} from "../utils";
import { hashPassword, verifyPassword, sha256Hex } from "../auth";
import { getUserById } from "../middleware";

export function publicUser(user: DbUser): Record<string, unknown> {
  return {
    name: `users/${user.id}`,
    id: user.id,
    username: user.username,
    role: user.role,
    email: user.email,
    nickname: user.nickname,
    avatarUrl: user.avatar_url,
    description: user.description,
    createdTs: user.created_ts,
    updatedTs: user.updated_ts
  };
}

export async function listUsers(env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const rows = await env.DB.prepare(`
    SELECT * FROM "user" ORDER BY id
  `).all<DbUser>();
  return json({ users: rows.results.map(publicUser) });
}

export async function getUser(env: Env, viewer: Viewer, identifier: string): Promise<Response> {
  let user: DbUser | null = null;
  if (/^\d+$/.test(identifier)) {
    user = await getUserById(env, Number(identifier));
  } else {
    user = await env.DB.prepare('SELECT * FROM "user" WHERE username = ?').bind(identifier).first<DbUser>();
  }
  if (!user) return json({ error: "User not found" }, 404);
  if (viewer.role !== "ADMIN" && viewer.id !== user.id) return json({ error: "Forbidden" }, 403);
  return json({ user: publicUser(user) });
}

export async function updateUser(request: Request, env: Env, viewer: Viewer, identifier: string): Promise<Response> {
  let user: DbUser | null = null;
  if (/^\d+$/.test(identifier)) {
    user = await getUserById(env, Number(identifier));
  } else {
    user = await env.DB.prepare('SELECT * FROM "user" WHERE username = ?').bind(identifier).first<DbUser>();
  }
  if (!user) return json({ error: "User not found" }, 404);
  if (viewer.role !== "ADMIN" && viewer.id !== user.id) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{
    email?: string;
    nickname?: string;
    description?: string;
    avatarUrl?: string;
    role?: string;
    rowStatus?: string;
  }>(request);

  const now = unixNow();
  const nextRole = body.role === "ADMIN" || body.role === "USER"
    ? (viewer.role === "ADMIN" ? body.role : user.role)
    : user.role;
  const nextRowStatus = body.rowStatus === "NORMAL" || body.rowStatus === "ARCHIVED"
    ? body.rowStatus
    : user.row_status;

  await env.DB.prepare(`
    UPDATE "user"
    SET updated_ts = ?, email = ?, nickname = ?, description = ?, avatar_url = ?, role = ?, row_status = ?
    WHERE id = ?
  `).bind(
    now,
    body.email !== undefined ? stringOrEmpty(body.email) : user.email,
    body.nickname !== undefined ? stringOrEmpty(body.nickname) : user.nickname,
    body.description !== undefined ? stringOrEmpty(body.description) : user.description,
    body.avatarUrl !== undefined ? stringOrEmpty(body.avatarUrl) : user.avatar_url,
    nextRole,
    nextRowStatus,
    user.id
  ).run();

  const updated = await getUserById(env, user.id);
  return json({ user: updated ? publicUser(updated) : null });
}

export async function deleteUser(env: Env, viewer: Viewer, identifier: string): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  let user: DbUser | null = null;
  if (/^\d+$/.test(identifier)) {
    user = await getUserById(env, Number(identifier));
  } else {
    user = await env.DB.prepare('SELECT * FROM "user" WHERE username = ?').bind(identifier).first<DbUser>();
  }
  if (!user) return json({ error: "User not found" }, 404);
  if (user.id === viewer.id) return json({ error: "Cannot delete yourself" }, 400);

  await env.DB.prepare('DELETE FROM "user" WHERE id = ?').bind(user.id).run();
  return json({ ok: true });
}

export async function updateMe(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const body = await readJson<{ email?: string; nickname?: string; description?: string; avatarUrl?: string }>(request);
  const now = unixNow();

  await env.DB.prepare(`
    UPDATE "user"
    SET updated_ts = ?, email = ?, nickname = ?, description = ?, avatar_url = ?
    WHERE id = ?
  `).bind(
    now,
    stringOrEmpty(body.email),
    stringOrEmpty(body.nickname),
    stringOrEmpty(body.description),
    stringOrEmpty(body.avatarUrl),
    viewer.id
  ).run();

  const user = await getUserById(env, viewer.id);
  return json({ user: user ? publicUser(user) : null });
}

export async function changePassword(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const body = await readJson<{ currentPassword?: string; newPassword?: string }>(request);
  if (!body.currentPassword) return json({ error: "Current password is required" }, 400);
  assertPassword(body.newPassword);

  const user = await getUserById(env, viewer.id);
  if (!user) return json({ error: "User not found" }, 404);
  if (!(await verifyPassword(body.currentPassword, user.password_hash))) {
    return json({ error: "Current password is incorrect" }, 401);
  }

  const now = unixNow();
  await env.DB.prepare('UPDATE "user" SET password_hash = ?, updated_ts = ? WHERE id = ?')
    .bind(await hashPassword(body.newPassword), now, viewer.id)
    .run();
  await env.DB.prepare("UPDATE user_session SET row_status = 'REVOKED', updated_ts = ? WHERE user_id = ?")
    .bind(now, viewer.id)
    .run();

  return json({ ok: true }, 200, [
    clearCookie(refreshCookieName),
    clearCookie(accessCookieName)
  ]);
}

// --- PAT Management ---

export async function listPats(env: Env, viewer: Viewer, userIdentifier: string): Promise<Response> {
  const user = await resolveUser(env, viewer, userIdentifier);
  if (!user) return json({ error: "User not found" }, 404);
  if (viewer.role !== "ADMIN" && viewer.id !== user.id) return json({ error: "Forbidden" }, 403);

  const rows = await env.DB.prepare(`
    SELECT id, name, token_prefix, created_ts, updated_ts, last_used_ts, expires_ts, row_status
    FROM user_access_token
    WHERE user_id = ?
    ORDER BY created_ts DESC
  `).bind(user.id).all<DbUserAccessToken>();

  return json({
    accessTokens: rows.results.map((row) => ({
      id: row.id,
      name: row.name,
      prefix: row.token_prefix,
      createdTs: row.created_ts,
      updatedTs: row.updated_ts,
      lastUsedTs: row.last_used_ts,
      expiresTs: row.expires_ts,
      rowStatus: row.row_status
    }))
  });
}

export async function createPat(request: Request, env: Env, viewer: Viewer, userIdentifier: string): Promise<Response> {
  const user = await resolveUser(env, viewer, userIdentifier);
  if (!user) return json({ error: "User not found" }, 404);
  if (viewer.role !== "ADMIN" && viewer.id !== user.id) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{ name?: string; expiresTs?: number }>(request);
  const name = String(body.name ?? "").trim() || "Unnamed Token";
  const now = unixNow();

  const rawToken = `memos_pat_${generateUid("")}`;
  const prefix = rawToken.slice(0, 20);
  const tokenHash = await sha256Hex(rawToken);

  const result = await env.DB.prepare(`
    INSERT INTO user_access_token (user_id, name, token_prefix, token_hash, created_ts, updated_ts, expires_ts)
    VALUES (?, ?, ?, ?, ?, ?, ?)
  `).bind(
    user.id,
    name,
    prefix,
    tokenHash,
    now,
    now,
    body.expiresTs && Number.isFinite(body.expiresTs) ? body.expiresTs : null
  ).run();

  return json({
    accessToken: {
      id: Number(result.meta.last_row_id),
      name,
      token: rawToken,
      prefix,
      createdTs: now,
      expiresTs: body.expiresTs ?? null
    }
  }, 201);
}

export async function deletePat(env: Env, viewer: Viewer, userIdentifier: string, patId: string): Promise<Response> {
  const user = await resolveUser(env, viewer, userIdentifier);
  if (!user) return json({ error: "User not found" }, 404);
  if (viewer.role !== "ADMIN" && viewer.id !== user.id) return json({ error: "Forbidden" }, 403);

  const id = Number(patId);
  if (!Number.isFinite(id)) return json({ error: "Invalid token ID" }, 400);

  const row = await env.DB.prepare(`
    SELECT id FROM user_access_token WHERE id = ? AND user_id = ?
  `).bind(id, user.id).first();
  if (!row) return json({ error: "Token not found" }, 404);

  await env.DB.prepare("DELETE FROM user_access_token WHERE id = ?").bind(id).run();
  return json({ ok: true });
}

// --- User Settings ---

export async function getUserSetting(env: Env, viewer: Viewer, userIdentifier: string, key: string): Promise<Response> {
  const user = await resolveUser(env, viewer, userIdentifier);
  if (!user) return json({ error: "User not found" }, 404);
  if (viewer.role !== "ADMIN" && viewer.id !== user.id) return json({ error: "Forbidden" }, 403);

  const row = await env.DB.prepare(`
    SELECT value FROM user_setting WHERE user_id = ? AND key = ?
  `).bind(user.id, key).first<{ value: string }>();

  return json({ key, value: row?.value ?? "" });
}

export async function listUserSettings(env: Env, viewer: Viewer, userIdentifier: string): Promise<Response> {
  const user = await resolveUser(env, viewer, userIdentifier);
  if (!user) return json({ error: "User not found" }, 404);
  if (viewer.role !== "ADMIN" && viewer.id !== user.id) return json({ error: "Forbidden" }, 403);

  const rows = await env.DB.prepare(`
    SELECT key, value FROM user_setting WHERE user_id = ?
  `).bind(user.id).all<DbUserSetting>();

  return json({ settings: rows.results });
}

export async function updateUserSetting(request: Request, env: Env, viewer: Viewer, userIdentifier: string, key: string): Promise<Response> {
  const user = await resolveUser(env, viewer, userIdentifier);
  if (!user) return json({ error: "User not found" }, 404);
  if (viewer.role !== "ADMIN" && viewer.id !== user.id) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{ value?: string }>(request);
  const value = String(body.value ?? "");

  await env.DB.prepare(`
    INSERT INTO user_setting (user_id, key, value) VALUES (?, ?, ?)
    ON CONFLICT (user_id, key) DO UPDATE SET value = excluded.value
  `).bind(user.id, key, value).run();

  return json({ key, value });
}

// --- User Stats ---

export async function getUserStats(env: Env, viewer: Viewer, userIdentifier: string): Promise<Response> {
  const user = await resolveUser(env, viewer, userIdentifier);
  if (!user) return json({ error: "User not found" }, 404);

  const memoCount = await env.DB.prepare(`
    SELECT COUNT(*) AS count FROM memo WHERE creator_id = ? AND row_status = 'NORMAL'
  `).bind(user.id).first<{ count: number }>();

  const attachmentCount = await env.DB.prepare(`
    SELECT COUNT(*) AS count FROM attachment WHERE creator_id = ?
  `).bind(user.id).first<{ count: number }>();

  return json({
    stats: {
      memoCount: memoCount?.count ?? 0,
      attachmentCount: attachmentCount?.count ?? 0
    }
  });
}

async function resolveUser(env: Env, viewer: Viewer, identifier: string): Promise<DbUser | null> {
  if (identifier === "me") return getUserById(env, viewer.id);
  if (/^\d+$/.test(identifier)) return getUserById(env, Number(identifier));
  return env.DB.prepare('SELECT * FROM "user" WHERE username = ?').bind(identifier).first<DbUser>();
}
