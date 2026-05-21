import type { Env, DbUser, Viewer } from "./types";
import { json, html, corsHeaders, readJson, unixNow, normalizeUsername, assertPassword, cookie, clearCookie, refreshCookieName, accessCookieName, parseCookies } from "./utils";
import { hashPassword, verifyPassword, signJwt, verifyJwt, sha256Hex } from "./auth";
import { currentViewer, getUserById } from "./middleware";
import { publicUser, listUsers, getUser, updateUser, deleteUser, updateMe, changePassword, listPats, createPat, deletePat, listUserSettings, getUserSetting, updateUserSetting, getUserStats } from "./services/user";
import { listMemos, createMemo, getMemo, updateMemo, deleteMemo, purgeMemo, bulkUpdateMemos, exportData, importData } from "./services/memo";
import { uploadAttachment, downloadAttachment, listAttachments, deleteAttachment, batchDeleteAttachments } from "./services/attachment";
import { createComment, listComments, upsertReaction, deleteReaction, listReactions, getRelations, setRelations } from "./services/social";
import { createShare, listShares, deleteShare, getSharedMemo, downloadSharedAttachment } from "./services/share";
import { listInbox, updateInboxStatus, deleteInboxItem } from "./services/inbox";
import { listWebhooks, createWebhook, updateWebhook, deleteWebhook, listWebhookDeliveries, retryWebhookDelivery, testWebhook } from "./services/webhook";
import { createBackupResponse, downloadBackup, listBackups, previewBackup, restoreBackup } from "./services/backup";
import { listTags, renameTag } from "./services/tags";
import { getTimeline } from "./services/timeline";
import { listAuditLogs } from "./services/audit";
import { importOriginalMemos, previewOriginalMemosMigration } from "./services/migration";
import { suggestMemoRelations } from "./services/aiRelations";
import { getAiSettings, testAiSettings, updateAiSettings } from "./services/aiSettings";
import { generateRss } from "./rss";
import { parseFilter } from "./filter";
import { appHtml } from "./ui";

export async function route(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  const method = request.method.toUpperCase();

  if (method === "OPTIONS") return new Response(null, { headers: corsHeaders() });

  // Public routes
  if (url.pathname === "/api/v1/instance" && method === "GET") return getInstance(env);
  if (url.pathname === "/api/v1/setup" && method === "POST") return setupAdmin(request, env);
  if (url.pathname === "/api/v1/auth/signup" && method === "POST") return signUp(request, env);
  if (url.pathname === "/api/v1/auth/signin" && method === "POST") return signIn(request, env);
  if (url.pathname === "/api/v1/auth/refresh" && method === "POST") return refreshSession(request, env);
  if (url.pathname === "/api/v1/auth/signout" && method === "POST") return signOut(request, env);

  // Public RSS
  if (url.pathname === "/api/v1/explore/rss.xml" && method === "GET") return generateRss(env);
  const userRssMatch = url.pathname.match(/^\/api\/v1\/u\/([^/]+)\/rss\.xml$/);
  if (userRssMatch && method === "GET") return generateRss(env, decodeURIComponent(userRssMatch[1]));

  // Public share access
  const shareTokenMatch = url.pathname.match(/^\/api\/v1\/shares\/([^/]+)$/);
  if (shareTokenMatch && method === "GET") return getSharedMemo(env, decodeURIComponent(shareTokenMatch[1]));
  const shareAttachmentMatch = url.pathname.match(/^\/api\/v1\/shares\/([^/]+)\/attachments\/([^/]+)\/(.+)$/);
  if (shareAttachmentMatch && method === "GET") {
    return downloadSharedAttachment(
      env,
      decodeURIComponent(shareAttachmentMatch[1]),
      decodeURIComponent(shareAttachmentMatch[2])
    );
  }

  if (url.pathname.startsWith("/api/") || url.pathname.startsWith("/file/")) {
    const viewer = await currentViewer(request, env);
    if (!viewer) return json({ error: "Unauthorized" }, 401);

    // Auth
    if (url.pathname === "/api/v1/auth/user" && method === "GET") return authUser(env, viewer);
    if (url.pathname === "/api/v1/auth/change-password" && method === "POST") return changePassword(request, env, viewer);
    if (url.pathname === "/api/v1/users/me" && method === "PATCH") return updateMe(request, env, viewer);
    if (url.pathname === "/api/v1/ai/settings" && method === "GET") return getAiSettings(env, viewer);
    if (url.pathname === "/api/v1/ai/settings" && method === "PATCH") return updateAiSettings(request, env, viewer);
    if (url.pathname === "/api/v1/ai/settings/test" && method === "POST") return testAiSettings(request, env, viewer);

    // Memos
    if (url.pathname === "/api/v1/memos" && method === "GET") {
      const filterExpr = url.searchParams.get("filter");
      let filterSql: { sql: string; params: unknown[] } | undefined;
      if (filterExpr) {
        const result = parseFilter(filterExpr);
        if (result) {
          filterSql = { sql: result.sql, params: result.params };
        }
      }
      return listMemos(request, env, viewer, filterSql);
    }
    if (url.pathname === "/api/v1/memos" && method === "POST") return createMemo(request, env, viewer);
    if (url.pathname === "/api/v1/memos/batch" && method === "POST") return bulkUpdateMemos(request, env, viewer);
    if (url.pathname === "/api/v1/export/memos" && method === "GET") return exportData(env, viewer);
    if (url.pathname === "/api/v1/import/memos" && method === "POST") return importData(request, env, viewer);
    if (url.pathname === "/api/v1/migration/memos/preview" && method === "POST") return previewOriginalMemosMigration(request, env, viewer);
    if (url.pathname === "/api/v1/migration/memos/import" && method === "POST") return importOriginalMemos(request, env, viewer);
    if (url.pathname === "/api/v1/backups" && method === "POST") {
      if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
      return createBackupResponse(env, viewer);
    }
    if (url.pathname === "/api/v1/backups" && method === "GET") {
      if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
      return listBackups(env, viewer);
    }
    if (url.pathname === "/api/v1/backups/download" && method === "GET") {
      if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
      return downloadBackup(request, env);
    }
    if (url.pathname === "/api/v1/backups/preview" && method === "POST") {
      if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
      return previewBackup(request, env);
    }
    if (url.pathname === "/api/v1/backups/restore" && method === "POST") {
      if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
      return restoreBackup(request, env, viewer);
    }

    // Attachments
    if (url.pathname === "/api/v1/attachments" && method === "POST") return uploadAttachment(request, env, viewer);
    if (url.pathname === "/api/v1/attachments" && method === "GET") return listAttachments(request, env, viewer);
    if (url.pathname === "/api/v1/attachments/batch-delete" && method === "POST") return batchDeleteAttachments(request, env, viewer);
    const attachmentDeleteMatch = url.pathname.match(/^\/api\/v1\/attachments\/([^/]+)$/);
    if (attachmentDeleteMatch && method === "DELETE") {
      return deleteAttachment(env, viewer, decodeURIComponent(attachmentDeleteMatch[1]));
    }

    // SSE
    if (url.pathname === "/api/v1/sse" && method === "GET") return connectSse(env, viewer);

    // Users
    if (url.pathname === "/api/v1/users" && method === "GET") return listUsers(env, viewer);
    if (url.pathname === "/api/v1/tags" && method === "GET") return listTags(env, viewer);
    if (url.pathname === "/api/v1/tags/rename" && method === "POST") return renameTag(request, env, viewer);
    if (url.pathname === "/api/v1/timeline" && method === "GET") return getTimeline(env, viewer);
    if (url.pathname === "/api/v1/audit-logs" && method === "GET") return listAuditLogs(env, viewer);

    // Memo by UID
    const memoMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)$/);
    if (memoMatch) {
      const uid = decodeURIComponent(memoMatch[1]);
      if (method === "GET") return getMemo(env, viewer, uid);
      if (method === "PATCH") return updateMemo(request, env, viewer, uid);
      if (method === "DELETE" && url.searchParams.get("purge") === "true") return purgeMemo(env, viewer, uid);
      if (method === "DELETE") return deleteMemo(env, viewer, uid);
    }

    // Memo comments
    const commentListMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)\/comments$/);
    if (commentListMatch) {
      const uid = decodeURIComponent(commentListMatch[1]);
      if (method === "GET") return listComments(env, viewer, uid);
      if (method === "POST") return createComment(request, env, viewer, uid);
    }

    // Memo reactions
    const reactionListMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)\/reactions$/);
    if (reactionListMatch) {
      const uid = decodeURIComponent(reactionListMatch[1]);
      if (method === "GET") return listReactions(env, viewer, uid);
      if (method === "POST") return upsertReaction(request, env, viewer, uid);
    }

    const reactionDeleteMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)\/reactions\/([^/]+)$/);
    if (reactionDeleteMatch && method === "DELETE") {
      return deleteReaction(env, viewer, decodeURIComponent(reactionDeleteMatch[1]), decodeURIComponent(reactionDeleteMatch[2]));
    }

    // Memo relations
    const relationSuggestMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)\/relations\/suggest$/);
    if (relationSuggestMatch && method === "POST") {
      return suggestMemoRelations(env, viewer, decodeURIComponent(relationSuggestMatch[1]));
    }

    const relationsMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)\/relations$/);
    if (relationsMatch) {
      const uid = decodeURIComponent(relationsMatch[1]);
      if (method === "GET") return getRelations(env, viewer, uid);
      if (method === "PATCH") return setRelations(request, env, viewer, uid);
    }

    // Memo shares
    const shareListMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)\/shares$/);
    if (shareListMatch) {
      const uid = decodeURIComponent(shareListMatch[1]);
      if (method === "GET") return listShares(env, viewer, uid);
      if (method === "POST") return createShare(request, env, viewer, uid);
    }

    const shareDeleteMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)\/shares\/([^/]+)$/);
    if (shareDeleteMatch && method === "DELETE") {
      return deleteShare(env, viewer, decodeURIComponent(shareDeleteMatch[1]), decodeURIComponent(shareDeleteMatch[2]));
    }

    // Inbox
    if (url.pathname === "/api/v1/inbox" && method === "GET") return listInbox(env, viewer);
    if (url.pathname === "/api/v1/inbox" && method === "PATCH") return updateInboxStatus(request, env, viewer);

    const inboxDeleteMatch = url.pathname.match(/^\/api\/v1\/inbox\/([^/]+)$/);
    if (inboxDeleteMatch && method === "DELETE") {
      return deleteInboxItem(env, viewer, decodeURIComponent(inboxDeleteMatch[1]));
    }

    // Webhooks
    if (url.pathname === "/api/v1/webhooks" && method === "GET") return listWebhooks(env, viewer);
    if (url.pathname === "/api/v1/webhooks" && method === "POST") return createWebhook(request, env, viewer);
    if (url.pathname === "/api/v1/webhooks/deliveries" && method === "GET") return listWebhookDeliveries(request, env, viewer);

    const webhookDeliveryRetryMatch = url.pathname.match(/^\/api\/v1\/webhooks\/deliveries\/([^/]+)\/retry$/);
    if (webhookDeliveryRetryMatch && method === "POST") {
      return retryWebhookDelivery(env, viewer, decodeURIComponent(webhookDeliveryRetryMatch[1]));
    }

    const webhookTestMatch = url.pathname.match(/^\/api\/v1\/webhooks\/([^/]+)\/test$/);
    if (webhookTestMatch && method === "POST") {
      return testWebhook(env, viewer, decodeURIComponent(webhookTestMatch[1]));
    }

    const webhookMatch = url.pathname.match(/^\/api\/v1\/webhooks\/([^/]+)$/);
    if (webhookMatch) {
      const id = decodeURIComponent(webhookMatch[1]);
      if (method === "PATCH") return updateWebhook(request, env, viewer, id);
      if (method === "DELETE") return deleteWebhook(env, viewer, id);
    }

    // User by identifier
    const userMatch = url.pathname.match(/^\/api\/v1\/users\/([^/]+)$/);
    if (userMatch) {
      const identifier = decodeURIComponent(userMatch[1]);
      if (method === "GET") return getUser(env, viewer, identifier);
      if (method === "PATCH") return updateUser(request, env, viewer, identifier);
      if (method === "DELETE") return deleteUser(env, viewer, identifier);
    }

    // User stats
    const userStatsMatch = url.pathname.match(/^\/api\/v1\/users\/([^/]+)\/stats$/);
    if (userStatsMatch && method === "GET") {
      return getUserStats(env, viewer, decodeURIComponent(userStatsMatch[1]));
    }

    // PAT management
    const patListMatch = url.pathname.match(/^\/api\/v1\/users\/([^/]+)\/access-tokens$/);
    if (patListMatch) {
      const identifier = decodeURIComponent(patListMatch[1]);
      if (method === "GET") return listPats(env, viewer, identifier);
      if (method === "POST") return createPat(request, env, viewer, identifier);
    }

    const patDeleteMatch = url.pathname.match(/^\/api\/v1\/users\/([^/]+)\/access-tokens\/([^/]+)$/);
    if (patDeleteMatch && method === "DELETE") {
      return deletePat(env, viewer, decodeURIComponent(patDeleteMatch[1]), decodeURIComponent(patDeleteMatch[2]));
    }

    // User settings
    const settingsListMatch = url.pathname.match(/^\/api\/v1\/users\/([^/]+)\/settings$/);
    if (settingsListMatch && method === "GET") {
      return listUserSettings(env, viewer, decodeURIComponent(settingsListMatch[1]));
    }

    const settingsMatch = url.pathname.match(/^\/api\/v1\/users\/([^/]+)\/settings\/([^/]+)$/);
    if (settingsMatch) {
      const identifier = decodeURIComponent(settingsMatch[1]);
      const key = decodeURIComponent(settingsMatch[2]);
      if (method === "GET") return getUserSetting(env, viewer, identifier, key);
      if (method === "PATCH") return updateUserSetting(request, env, viewer, identifier, key);
    }

    // Attachment download
    const attachmentMatch = url.pathname.match(/^\/file\/attachments\/([^/]+)\/(.+)$/);
    if (attachmentMatch && method === "GET") {
      return downloadAttachment(env, viewer, decodeURIComponent(attachmentMatch[1]));
    }

    return json({ error: "Not found" }, 404);
  }

  // Static assets (frontend SPA)
  if (env.ASSETS) {
    const assetResponse = await env.ASSETS.fetch(request);
    if (assetResponse.status !== 404) return assetResponse;
    // SPA fallback: serve index.html for all non-API, non-file routes
    const indexReq = new Request(new URL("/index.html", request.url).toString(), request);
    return env.ASSETS.fetch(indexReq);
  }
  return html(appHtml());
}

async function getInstance(env: Env): Promise<Response> {
  const count = await env.DB.prepare('SELECT COUNT(*) AS count FROM "user"').first<{ count: number }>();
  return json({
    name: "Memos Worker",
    setupRequired: !count || count.count === 0
  });
}

async function setupAdmin(request: Request, env: Env): Promise<Response> {
  const existing = await env.DB.prepare('SELECT COUNT(*) AS count FROM "user"').first<{ count: number }>();
  if (existing && existing.count > 0) return json({ error: "Instance already initialized" }, 409);

  const body = await readJson<{
    username?: string;
    password?: string;
    email?: string;
    nickname?: string;
  }>(request);

  const username = normalizeUsername(body.username);
  assertPassword(body.password);

  const passwordHash = await hashPassword(body.password);
  const now = unixNow();
  const result = await env.DB.prepare(`
    INSERT INTO "user" (created_ts, updated_ts, username, role, email, nickname, password_hash)
    VALUES (?, ?, ?, 'ADMIN', ?, ?, ?)
  `).bind(now, now, username, body.email ?? "", body.nickname ?? username, passwordHash).run();

  const user = await getUserById(env, Number(result.meta.last_row_id));
  if (!user) return json({ error: "Failed to create admin" }, 500);
  return createAuthResponse(env, request, user, 201);
}

async function signIn(request: Request, env: Env): Promise<Response> {
  const body = await readJson<{ username?: string; password?: string }>(request);
  const username = normalizeUsername(body.username);
  if (!body.password) return json({ error: "Password is required" }, 400);

  const user = await env.DB.prepare('SELECT * FROM "user" WHERE username = ? AND row_status = ?')
    .bind(username, "NORMAL")
    .first<DbUser>();
  if (!user) return json({ error: "Invalid username or password" }, 401);

  if (!(await verifyPassword(body.password, user.password_hash))) {
    return json({ error: "Invalid username or password" }, 401);
  }

  return createAuthResponse(env, request, user);
}

async function signUp(request: Request, env: Env): Promise<Response> {
  const body = await readJson<{
    username?: string;
    password?: string;
    nickname?: string;
  }>(request);

  const username = normalizeUsername(body.username);
  assertPassword(body.password);

  const existing = await env.DB.prepare('SELECT id FROM "user" WHERE username = ?').bind(username).first();
  if (existing) return json({ error: "用户名已存在" }, 409);

  const passwordHash = await hashPassword(body.password);
  const now = unixNow();
  const result = await env.DB.prepare(`
    INSERT INTO "user" (created_ts, updated_ts, username, role, email, nickname, password_hash)
    VALUES (?, ?, ?, 'USER', '', ?, ?)
  `).bind(now, now, username, body.nickname ?? username, passwordHash).run();

  const user = await getUserById(env, Number(result.meta.last_row_id));
  if (!user) return json({ error: "注册失败" }, 500);
  return createAuthResponse(env, request, user, 201);
}

async function refreshSession(request: Request, env: Env): Promise<Response> {
  const cookies = parseCookies(request.headers.get("Cookie"));
  const refreshToken = cookies.get(refreshCookieName);
  if (!refreshToken) return json({ error: "Missing refresh token" }, 401);

  const claims = await verifyJwt(refreshToken, env.SERVER_SECRET);
  if (!claims || claims.type !== "refresh" || !claims.tid) return json({ error: "Invalid refresh token" }, 401);

  const tokenHash = await sha256Hex(refreshToken);
  const session = await env.DB.prepare(`
    SELECT * FROM user_session
    WHERE id = ? AND refresh_token_hash = ? AND row_status = 'NORMAL' AND expires_ts > ?
  `).bind(claims.tid, tokenHash, unixNow()).first<{ user_id: number }>();
  if (!session) return json({ error: "Refresh token revoked or expired" }, 401);

  await env.DB.prepare("UPDATE user_session SET row_status = 'REVOKED', updated_ts = ? WHERE id = ?")
    .bind(unixNow(), claims.tid)
    .run();

  const user = await getUserById(env, session.user_id);
  if (!user || user.row_status !== "NORMAL") return json({ error: "User unavailable" }, 401);
  return createAuthResponse(env, request, user);
}

async function signOut(request: Request, env: Env): Promise<Response> {
  const cookies = parseCookies(request.headers.get("Cookie"));
  const refreshToken = cookies.get(refreshCookieName);
  if (refreshToken) {
    const hash = await sha256Hex(refreshToken);
    await env.DB.prepare("UPDATE user_session SET row_status = 'REVOKED', updated_ts = ? WHERE refresh_token_hash = ?")
      .bind(unixNow(), hash)
      .run();
  }

  return json({ ok: true }, 200, [
    clearCookie(refreshCookieName),
    clearCookie(accessCookieName)
  ]);
}

async function authUser(env: Env, viewer: Viewer): Promise<Response> {
  const user = await getUserById(env, viewer.id);
  if (!user) return json({ error: "User not found" }, 404);
  return json({ user: publicUser(user) });
}

async function connectSse(env: Env, viewer: Viewer): Promise<Response> {
  const token = await signJwt({
    type: "sse",
    role: viewer.role,
    status: viewer.rowStatus,
    username: viewer.username,
    iss: "memos-worker",
    aud: ["memos.sse"],
    sub: String(viewer.id),
    iat: unixNow(),
    exp: unixNow() + 60
  }, env.SERVER_SECRET);
  const id = env.SSE_HUB.idFromName("global");
  return env.SSE_HUB.get(id).fetch(`https://sse-hub/connect?token=${encodeURIComponent(token)}`);
}

async function createAuthResponse(env: Env, request: Request, user: DbUser, status = 200): Promise<Response> {
  const now = unixNow();
  const accessToken = await signJwt({
    type: "access",
    role: user.role,
    status: user.row_status,
    username: user.username,
    iss: "memos-worker",
    aud: ["user.access-token"],
    sub: String(user.id),
    iat: now,
    exp: now + 15 * 60
  }, env.SERVER_SECRET);

  const sessionId = crypto.randomUUID();
  const refreshToken = await signJwt({
    type: "refresh",
    tid: sessionId,
    role: user.role,
    status: user.row_status,
    username: user.username,
    iss: "memos-worker",
    aud: ["user.refresh-token"],
    sub: String(user.id),
    iat: now,
    exp: now + 30 * 24 * 60 * 60
  }, env.SERVER_SECRET);

  await env.DB.prepare(`
    INSERT INTO user_session (id, user_id, refresh_token_hash, created_ts, updated_ts, last_used_ts, expires_ts, user_agent, ip_address)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
  `).bind(
    sessionId,
    user.id,
    await sha256Hex(refreshToken),
    now,
    now,
    now,
    now + 30 * 24 * 60 * 60,
    request.headers.get("User-Agent") ?? "",
    request.headers.get("CF-Connecting-IP") ?? ""
  ).run();

  return json({ accessToken, user: publicUser(user) }, status, [
    cookie(refreshCookieName, refreshToken, 30 * 24 * 60 * 60, true),
    cookie(accessCookieName, accessToken, 15 * 60, true)
  ]);
}
