export interface Env {
  DB: D1Database;
  MEMOS_BUCKET: R2Bucket;
  SSE_HUB: DurableObjectNamespace;
  SERVER_SECRET: string;
  ENVIRONMENT?: string;
}

type Role = "ADMIN" | "USER";
type RowStatus = "NORMAL" | "ARCHIVED";
type Visibility = "PUBLIC" | "PROTECTED" | "PRIVATE";

interface DbUser {
  id: number;
  created_ts: number;
  updated_ts: number;
  row_status: RowStatus;
  username: string;
  role: Role;
  email: string;
  nickname: string;
  password_hash: string;
  avatar_url: string;
  description: string;
}

interface DbMemo {
  id: number;
  uid: string;
  creator_id: number;
  created_ts: number;
  updated_ts: number;
  row_status: RowStatus;
  content: string;
  visibility: Visibility;
  pinned: number;
  payload: string;
  creator_username?: string;
  creator_nickname?: string;
}

interface DbAttachment {
  id: number;
  uid: string;
  creator_id: number;
  created_ts: number;
  updated_ts: number;
  filename: string;
  type: string;
  size: number;
  memo_id: number | null;
  storage_type: "S3" | "DATABASE" | "LOCAL";
  reference: string;
  payload: string;
  memo_visibility?: Visibility | null;
  memo_creator_id?: number | null;
}

interface Claims {
  type: "access" | "refresh" | "sse";
  role: Role;
  status: RowStatus;
  username: string;
  iss: "memos-worker";
  aud: string[];
  sub: string;
  tid?: string;
  iat: number;
  exp: number;
}

interface Viewer {
  id: number;
  username: string;
  role: Role;
  rowStatus: RowStatus;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const refreshCookieName = "memos_refresh";
const accessCookieName = "memos_access";

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    try {
      return await route(request, env);
    } catch (error) {
      if (error instanceof HttpError) {
        return json({ error: error.message }, error.status);
      }
      console.error(error);
      return json({ error: "Internal server error" }, 500);
    }
  }
};

export class SSEHub {
  private sessions = new Map<string, {
    userId: number;
    role: Role;
    writer: WritableStreamDefaultWriter<Uint8Array>;
  }>();

  constructor(private state: DurableObjectState, private env: Env) {}

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "POST" && url.pathname === "/broadcast") {
      const event = await request.json<SseEvent>();
      await this.broadcast(event);
      return json({ ok: true });
    }

    if (request.method !== "GET" || url.pathname !== "/connect") {
      return json({ error: "Not found" }, 404);
    }

    const token = url.searchParams.get("token");
    if (!token) return json({ error: "Missing token" }, 401);

    const claims = await verifyJwt(token, this.env.SERVER_SECRET);
    if (!claims || claims.type !== "sse") return json({ error: "Invalid token" }, 401);

    const sessionId = crypto.randomUUID();
    const stream = new TransformStream<Uint8Array, Uint8Array>();
    const writer = stream.writable.getWriter();

    this.sessions.set(sessionId, {
      userId: Number(claims.sub),
      role: claims.role,
      writer
    });

    const heartbeat = setInterval(() => {
      writer.write(encoder.encode(`: heartbeat\n\n`)).catch(() => {
        clearInterval(heartbeat);
        this.sessions.delete(sessionId);
      });
    }, 30000);

    writer.write(encoder.encode(`event: ready\ndata: {}\n\n`)).catch(() => {
      clearInterval(heartbeat);
      this.sessions.delete(sessionId);
    });

    request.signal.addEventListener("abort", () => {
      clearInterval(heartbeat);
      this.sessions.delete(sessionId);
      writer.close().catch(() => undefined);
    });

    return new Response(stream.readable, {
      headers: {
        "Content-Type": "text/event-stream; charset=utf-8",
        "Cache-Control": "no-cache, no-transform",
        "X-Accel-Buffering": "no"
      }
    });
  }

  private async broadcast(event: SseEvent): Promise<void> {
    const chunk = encoder.encode(
      `id: ${event.id}\nevent: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`
    );

    const writes: Promise<unknown>[] = [];
    for (const [sessionId, session] of this.sessions) {
      if (!canReceiveEvent(event, session.userId, session.role)) continue;
      writes.push(session.writer.write(chunk).catch(() => {
        this.sessions.delete(sessionId);
      }));
    }
    await Promise.all(writes);
  }
}

interface SseEvent {
  id: string;
  type: "memo.created" | "memo.updated" | "memo.deleted";
  name: string;
  visibility: Visibility;
  creatorId: number;
}

async function route(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  const method = request.method.toUpperCase();

  if (method === "OPTIONS") return new Response(null, { headers: corsHeaders() });

  if (url.pathname === "/api/v1/instance" && method === "GET") return getInstance(env);
  if (url.pathname === "/api/v1/setup" && method === "POST") return setupAdmin(request, env);
  if (url.pathname === "/api/v1/auth/signin" && method === "POST") return signIn(request, env);
  if (url.pathname === "/api/v1/auth/refresh" && method === "POST") return refreshSession(request, env);
  if (url.pathname === "/api/v1/auth/signout" && method === "POST") return signOut(request, env);

  if (url.pathname.startsWith("/api/") || url.pathname.startsWith("/file/")) {
    const viewer = await currentViewer(request, env);
    if (!viewer) return json({ error: "Unauthorized" }, 401);

    if (url.pathname === "/api/v1/auth/user" && method === "GET") return authUser(env, viewer);
    if (url.pathname === "/api/v1/auth/change-password" && method === "POST") return changePassword(request, env, viewer);
    if (url.pathname === "/api/v1/users/me" && method === "PATCH") return updateMe(request, env, viewer);
    if (url.pathname === "/api/v1/memos" && method === "GET") return listMemos(request, env, viewer);
    if (url.pathname === "/api/v1/memos" && method === "POST") return createMemo(request, env, viewer);
    if (url.pathname === "/api/v1/export/memos" && method === "GET") return exportData(env, viewer);
    if (url.pathname === "/api/v1/import/memos" && method === "POST") return importData(request, env, viewer);
    if (url.pathname === "/api/v1/attachments" && method === "POST") return uploadAttachment(request, env, viewer);
    if (url.pathname === "/api/v1/sse" && method === "GET") return connectSse(env, viewer);

    const memoMatch = url.pathname.match(/^\/api\/v1\/memos\/([^/]+)$/);
    if (memoMatch) {
      const uid = decodeURIComponent(memoMatch[1]);
      if (method === "GET") return getMemo(env, viewer, uid);
      if (method === "PATCH") return updateMemo(request, env, viewer, uid);
      if (method === "DELETE") return deleteMemo(env, viewer, uid);
    }

    const attachmentMatch = url.pathname.match(/^\/file\/attachments\/([^/]+)\/(.+)$/);
    if (attachmentMatch && method === "GET") {
      return downloadAttachment(env, viewer, decodeURIComponent(attachmentMatch[1]));
    }

    return json({ error: "Not found" }, 404);
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
  if (!user || !(await verifyPassword(body.password, user.password_hash))) {
    return json({ error: "Invalid username or password" }, 401);
  }

  return createAuthResponse(env, request, user);
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

async function changePassword(request: Request, env: Env, viewer: Viewer): Promise<Response> {
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

async function updateMe(request: Request, env: Env, viewer: Viewer): Promise<Response> {
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

async function listMemos(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const url = new URL(request.url);
  const pageSize = clampNumber(Number(url.searchParams.get("page_size") ?? 50), 1, 100);
  const state = normalizeState(url.searchParams.get("state"));
  const visibility = normalizeVisibility(url.searchParams.get("visibility"), true);
  const tag = url.searchParams.get("tag")?.trim();
  const cursor = decodePageToken(url.searchParams.get("page_token"));

  const where: string[] = ["memo.row_status = ?"];
  const params: unknown[] = [state];

  if (viewer.role !== "ADMIN") {
    where.push("(memo.visibility != 'PRIVATE' OR memo.creator_id = ?)");
    params.push(viewer.id);
  }

  if (visibility) {
    where.push("memo.visibility = ?");
    params.push(visibility);
  }

  if (tag) {
    where.push("EXISTS (SELECT 1 FROM json_each(json_extract(memo.payload, '$.tags')) WHERE value = ?)");
    params.push(tag);
  }

  if (cursor) {
    where.push("(memo.created_ts < ? OR (memo.created_ts = ? AND memo.id < ?))");
    params.push(cursor.createdTs, cursor.createdTs, cursor.id);
  }

  const rows = await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN "user" ON "user".id = memo.creator_id
    WHERE ${where.join(" AND ")}
    ORDER BY memo.pinned DESC, memo.created_ts DESC, memo.id DESC
    LIMIT ?
  `).bind(...params, pageSize + 1).all<DbMemo>();

  const memos = rows.results.slice(0, pageSize);
  const last = memos[memos.length - 1];
  return json({
    memos: await Promise.all(memos.map((memo) => memoWithAttachments(env, memo))),
    nextPageToken: rows.results.length > pageSize && last ? encodePageToken(last.created_ts, last.id) : ""
  });
}

async function createMemo(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const body = await readJson<{ content?: string; visibility?: string; pinned?: boolean; attachmentUids?: string[] }>(request);
  const content = String(body.content ?? "").trim();
  if (!content) return json({ error: "Content is required" }, 400);

  const visibility = normalizeVisibility(body.visibility, false) ?? "PRIVATE";
  const uid = generateUid("m");
  const now = unixNow();
  const payload = buildMemoPayload(content);

  const result = await env.DB.prepare(`
    INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, pinned, payload)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
  `).bind(uid, viewer.id, now, now, content, visibility, body.pinned ? 1 : 0, JSON.stringify(payload)).run();

  const memo = await getMemoById(env, Number(result.meta.last_row_id));
  if (body.attachmentUids?.length) {
    await attachFiles(env, viewer, Number(result.meta.last_row_id), body.attachmentUids);
  }
  if (memo) await broadcastMemo(env, "memo.created", memo);
  return json({ memo: memo ? await memoWithAttachments(env, memo) : null }, 201);
}

async function getMemo(env: Env, viewer: Viewer, uid: string): Promise<Response> {
  const memo = await getMemoByUid(env, uid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);
  return json({ memo: await memoWithAttachments(env, memo) });
}

async function updateMemo(request: Request, env: Env, viewer: Viewer, uid: string): Promise<Response> {
  const memo = await getMemoByUid(env, uid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canWriteMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  const body = await readJson<{
    content?: string;
    visibility?: string;
    pinned?: boolean;
    rowStatus?: string;
    attachmentUids?: string[];
  }>(request);

  const nextContent = body.content === undefined ? memo.content : String(body.content).trim();
  if (!nextContent) return json({ error: "Content is required" }, 400);

  const nextVisibility = body.visibility === undefined
    ? memo.visibility
    : normalizeVisibility(body.visibility, false) ?? memo.visibility;
  const nextStatus = body.rowStatus === undefined ? memo.row_status : normalizeState(body.rowStatus);
  const nextPinned = body.pinned === undefined ? memo.pinned : body.pinned ? 1 : 0;

  await env.DB.prepare(`
    UPDATE memo
    SET updated_ts = ?, content = ?, visibility = ?, pinned = ?, row_status = ?, payload = ?
    WHERE id = ?
  `).bind(unixNow(), nextContent, nextVisibility, nextPinned, nextStatus, JSON.stringify(buildMemoPayload(nextContent)), memo.id).run();

  if (body.attachmentUids) await attachFiles(env, viewer, memo.id, body.attachmentUids);

  const updated = await getMemoByUid(env, uid);
  if (updated) await broadcastMemo(env, "memo.updated", updated);
  return json({ memo: updated ? await memoWithAttachments(env, updated) : null });
}

async function deleteMemo(env: Env, viewer: Viewer, uid: string): Promise<Response> {
  const memo = await getMemoByUid(env, uid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canWriteMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  await env.DB.prepare("UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id = ?")
    .bind(unixNow(), memo.id)
    .run();
  await broadcastMemo(env, "memo.deleted", memo);
  return json({ ok: true });
}

async function uploadAttachment(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const form = await request.formData();
  const file = form.get("file");
  const memoUid = String(form.get("memoUid") ?? "");
  if (!(file instanceof File)) return json({ error: "file is required" }, 400);
  if (file.size > 25 * 1024 * 1024) return json({ error: "file is too large" }, 413);

  let memoId: number | null = null;
  if (memoUid) {
    const memo = await getMemoByUid(env, memoUid);
    if (!memo) return json({ error: "Memo not found" }, 404);
    if (!canWriteMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);
    memoId = memo.id;
  }

  const uid = generateUid("a");
  const filename = sanitizeFilename(file.name || "attachment");
  const key = `attachments/${viewer.id}/${uid}/${filename}`;

  await env.MEMOS_BUCKET.put(key, file.stream(), {
    httpMetadata: {
      contentType: file.type || "application/octet-stream",
      contentDisposition: `inline; filename="${filename}"`
    },
    customMetadata: {
      creatorId: String(viewer.id),
      originalFilename: file.name || filename
    }
  });

  const result = await env.DB.prepare(`
    INSERT INTO attachment (uid, creator_id, filename, type, size, memo_id, storage_type, reference, payload)
    VALUES (?, ?, ?, ?, ?, ?, 'S3', ?, '{}')
  `).bind(uid, viewer.id, filename, file.type || "application/octet-stream", file.size, memoId, key).run();

  const attachment = await getAttachmentById(env, Number(result.meta.last_row_id));
  return json({ attachment: attachment ? publicAttachment(attachment) : null }, 201);
}

async function downloadAttachment(env: Env, viewer: Viewer, uid: string): Promise<Response> {
  const attachment = await getAttachmentByUid(env, uid);
  if (!attachment) return json({ error: "Attachment not found" }, 404);
  if (!canReadAttachment(attachment, viewer)) return json({ error: "Forbidden" }, 403);

  const object = await env.MEMOS_BUCKET.get(attachment.reference);
  if (!object) return json({ error: "File not found" }, 404);

  return new Response(object.body, {
    headers: {
      "Content-Type": attachment.type || object.httpMetadata?.contentType || "application/octet-stream",
      "Content-Disposition": `inline; filename="${attachment.filename}"`,
      "Cache-Control": attachment.memo_visibility === "PUBLIC" ? "public, max-age=3600" : "private, no-store"
    }
  });
}

async function exportData(env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const users = await env.DB.prepare(`
    SELECT id, created_ts, updated_ts, row_status, username, role, email, nickname, avatar_url, description
    FROM "user"
    ORDER BY id
  `).all();
  const memos = await env.DB.prepare("SELECT * FROM memo ORDER BY created_ts, id").all();
  const attachments = await env.DB.prepare(`
    SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload
    FROM attachment
    ORDER BY created_ts, id
  `).all();

  return json({
    exportedAt: new Date().toISOString(),
    users: users.results,
    memos: memos.results,
    attachments: attachments.results
  });
}

async function importData(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const body = await readJson<{
    memos?: Array<Partial<DbMemo> & { createdAt?: string; tags?: string[] }>;
  }>(request);

  let imported = 0;
  const now = unixNow();

  for (const item of body.memos ?? []) {
    const content = String(item.content ?? "").trim();
    if (!content) continue;
    const uid = item.uid || generateUid("m");
    const createdTs = Number(item.created_ts ?? (item.createdAt ? Math.floor(Date.parse(item.createdAt) / 1000) : now));
    const visibility = normalizeVisibility(item.visibility, false) ?? "PRIVATE";
    const payload = item.payload ? safeJsonParse(item.payload, buildMemoPayload(content)) : buildMemoPayload(content, item.tags);

    await env.DB.prepare(`
      INSERT OR IGNORE INTO memo (uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).bind(
      uid,
      viewer.id,
      Number.isFinite(createdTs) ? createdTs : now,
      Number(item.updated_ts ?? createdTs ?? now),
      normalizeState(item.row_status),
      content,
      visibility,
      item.pinned ? 1 : 0,
      JSON.stringify(payload)
    ).run();
    imported += 1;
  }

  return json({ imported });
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

async function currentViewer(request: Request, env: Env): Promise<Viewer | null> {
  const auth = request.headers.get("Authorization");
  let claims: Claims | null = null;

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

async function viewerFromPat(env: Env, token: string): Promise<Viewer | null> {
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

async function getUserById(env: Env, id: number): Promise<DbUser | null> {
  return await env.DB.prepare('SELECT * FROM "user" WHERE id = ?').bind(id).first<DbUser>();
}

async function getMemoById(env: Env, id: number): Promise<DbMemo | null> {
  return await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN "user" ON "user".id = memo.creator_id
    WHERE memo.id = ?
  `).bind(id).first<DbMemo>();
}

async function getMemoByUid(env: Env, uid: string): Promise<DbMemo | null> {
  return await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN "user" ON "user".id = memo.creator_id
    WHERE memo.uid = ?
  `).bind(uid).first<DbMemo>();
}

async function getAttachmentById(env: Env, id: number): Promise<DbAttachment | null> {
  return await env.DB.prepare(`
    SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id
    FROM attachment
    LEFT JOIN memo ON memo.id = attachment.memo_id
    WHERE attachment.id = ?
  `).bind(id).first<DbAttachment>();
}

async function getAttachmentByUid(env: Env, uid: string): Promise<DbAttachment | null> {
  return await env.DB.prepare(`
    SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id
    FROM attachment
    LEFT JOIN memo ON memo.id = attachment.memo_id
    WHERE attachment.uid = ?
  `).bind(uid).first<DbAttachment>();
}

async function attachFiles(env: Env, viewer: Viewer, memoId: number, attachmentUids: string[]): Promise<void> {
  for (const uid of attachmentUids.slice(0, 20)) {
    await env.DB.prepare(`
      UPDATE attachment SET memo_id = ?, updated_ts = ?
      WHERE uid = ? AND creator_id = ?
    `).bind(memoId, unixNow(), uid, viewer.id).run();
  }
}

function publicUser(user: DbUser): Record<string, unknown> {
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

function publicMemo(memo: DbMemo): Record<string, unknown> {
  return {
    name: `memos/${memo.uid}`,
    id: memo.id,
    uid: memo.uid,
    creator: {
      id: memo.creator_id,
      username: memo.creator_username,
      nickname: memo.creator_nickname
    },
    createdTs: memo.created_ts,
    updatedTs: memo.updated_ts,
    rowStatus: memo.row_status,
    content: memo.content,
    visibility: memo.visibility,
    pinned: Boolean(memo.pinned),
    payload: safeJsonParse(memo.payload, {})
  };
}

async function memoWithAttachments(env: Env, memo: DbMemo): Promise<Record<string, unknown>> {
  const attachments = await env.DB.prepare(`
    SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id
    FROM attachment
    LEFT JOIN memo ON memo.id = attachment.memo_id
    WHERE attachment.memo_id = ?
    ORDER BY attachment.created_ts, attachment.id
  `).bind(memo.id).all<DbAttachment>();

  return {
    ...publicMemo(memo),
    attachments: attachments.results.map(publicAttachment)
  };
}

function publicAttachment(attachment: DbAttachment): Record<string, unknown> {
  return {
    name: `attachments/${attachment.uid}`,
    uid: attachment.uid,
    filename: attachment.filename,
    type: attachment.type,
    size: attachment.size,
    memoId: attachment.memo_id,
    createdTs: attachment.created_ts,
    url: `/file/attachments/${encodeURIComponent(attachment.uid)}/${encodeURIComponent(attachment.filename)}`
  };
}

function canReadMemo(memo: DbMemo, viewer: Viewer): boolean {
  if (viewer.role === "ADMIN") return true;
  if (memo.visibility !== "PRIVATE") return true;
  return memo.creator_id === viewer.id;
}

function canWriteMemo(memo: DbMemo, viewer: Viewer): boolean {
  return viewer.role === "ADMIN" || memo.creator_id === viewer.id;
}

function canReadAttachment(attachment: DbAttachment, viewer: Viewer): boolean {
  if (viewer.role === "ADMIN") return true;
  if (!attachment.memo_id) return attachment.creator_id === viewer.id;
  if (attachment.memo_visibility !== "PRIVATE") return true;
  return attachment.memo_creator_id === viewer.id;
}

function canReceiveEvent(event: SseEvent, userId: number, role: Role): boolean {
  if (role === "ADMIN") return true;
  if (event.visibility !== "PRIVATE") return true;
  return event.creatorId === userId;
}

async function broadcastMemo(env: Env, type: SseEvent["type"], memo: DbMemo): Promise<void> {
  const id = env.SSE_HUB.idFromName("global");
  const event: SseEvent = {
    id: `${Date.now()}-${memo.uid}`,
    type,
    name: `memos/${memo.uid}`,
    visibility: memo.visibility,
    creatorId: memo.creator_id
  };
  await env.SSE_HUB.get(id).fetch("https://sse-hub/broadcast", {
    method: "POST",
    body: JSON.stringify(event),
    headers: { "Content-Type": "application/json" }
  }).catch((error) => {
    console.warn("SSE broadcast failed", error);
  });
}

export async function hashPassword(password: string, iterations = 310000): Promise<string> {
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

async function signJwt(claims: Claims, secret: string): Promise<string> {
  const header = { alg: "HS256", typ: "JWT" };
  const payload = base64url(encoder.encode(JSON.stringify(claims)));
  const protectedHeader = base64url(encoder.encode(JSON.stringify(header)));
  const data = `${protectedHeader}.${payload}`;
  const signature = await hmacSha256(data, secret);
  return `${data}.${base64url(signature)}`;
}

async function verifyJwt(token: string, secret: string): Promise<Claims | null> {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  const [header, payload, signature] = parts;
  const expected = await hmacSha256(`${header}.${payload}`, secret);
  if (!constantTimeEqual(fromBase64url(signature), expected)) return null;

  const claims = JSON.parse(decoder.decode(fromBase64url(payload))) as Claims;
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

async function sha256Hex(value: string): Promise<string> {
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", encoder.encode(value)));
  return [...hash].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function base64url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function fromBase64url(value: string): Uint8Array {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(value.length / 4) * 4, "=");
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  return bytes;
}

function constantTimeEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let index = 0; index < a.length; index += 1) diff |= a[index] ^ b[index];
  return diff === 0;
}

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function json(body: unknown, status = 200, cookies: string[] = []): Response {
  const headers = new Headers(corsHeaders());
  headers.set("Content-Type", "application/json; charset=utf-8");
  for (const value of cookies) headers.append("Set-Cookie", value);
  return new Response(JSON.stringify(body), { status, headers });
}

function html(body: string): Response {
  return new Response(body, {
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store"
    }
  });
}

function corsHeaders(): HeadersInit {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET,POST,PATCH,DELETE,OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type,Authorization"
  };
}

async function readJson<T>(request: Request): Promise<T> {
  try {
    return await request.json<T>();
  } catch {
    throw new HttpError("Invalid JSON", 400);
  }
}

class HttpError extends Error {
  constructor(message: string, public status: number) {
    super(message);
  }
}

function parseCookies(header: string | null): Map<string, string> {
  const cookies = new Map<string, string>();
  if (!header) return cookies;
  for (const part of header.split(";")) {
    const [name, ...rest] = part.trim().split("=");
    if (name) cookies.set(name, decodeURIComponent(rest.join("=")));
  }
  return cookies;
}

function cookie(name: string, value: string, maxAge: number, httpOnly: boolean): string {
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

function clearCookie(name: string): string {
  return `${name}=; Path=/api/v1; Max-Age=0; SameSite=Lax; Secure; HttpOnly`;
}

function normalizeUsername(value: unknown): string {
  const username = String(value ?? "").trim().toLowerCase();
  if (!/^[a-z0-9_][a-z0-9_-]{2,31}$/.test(username)) {
    throw new HttpError("Username must be 3-32 lowercase letters, numbers, _ or -", 400);
  }
  return username;
}

function assertPassword(value: unknown): asserts value is string {
  if (typeof value !== "string" || value.length < 8) {
    throw new HttpError("Password must be at least 8 characters", 400);
  }
}

function normalizeVisibility(value: unknown, allowEmpty: boolean): Visibility | null {
  if ((value === null || value === undefined || value === "") && allowEmpty) return null;
  const visibility = String(value ?? "").toUpperCase();
  if (visibility === "PUBLIC" || visibility === "PROTECTED" || visibility === "PRIVATE") return visibility;
  if (allowEmpty) return null;
  throw new HttpError("Invalid visibility", 400);
}

function normalizeState(value: unknown): RowStatus {
  const state = String(value ?? "NORMAL").toUpperCase();
  if (state === "NORMAL" || state === "ARCHIVED") return state;
  throw new HttpError("Invalid row status", 400);
}

function stringOrEmpty(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function generateUid(prefix: string): string {
  return `${prefix}_${base64url(crypto.getRandomValues(new Uint8Array(12)))}`;
}

export function buildMemoPayload(content: string, explicitTags?: string[]): Record<string, unknown> {
  const tags = explicitTags ?? extractTags(content);
  return {
    tags,
    property: {
      hasTaskList: /(^|\n)\s*[-*]\s+\[[ xX]\]/.test(content),
      hasLink: /https?:\/\/\S+/.test(content),
      hasCode: /```|`[^`]+`/.test(content),
      hasIncompleteTasks: /(^|\n)\s*[-*]\s+\[\s\]/.test(content)
    }
  };
}

function extractTags(content: string): string[] {
  const tags = new Set<string>();
  const regex = /(^|\s)#([\p{L}\p{N}_/-]{1,64})/gu;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(content))) tags.add(match[2]);
  return [...tags];
}

function safeJsonParse<T>(value: string, fallback: T): T {
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

function unixNow(): number {
  return Math.floor(Date.now() / 1000);
}

function clampNumber(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(Math.max(Math.floor(value), min), max);
}

function encodePageToken(createdTs: number, id: number): string {
  return base64url(encoder.encode(JSON.stringify({ createdTs, id })));
}

function decodePageToken(token: string | null): { createdTs: number; id: number } | null {
  if (!token) return null;
  try {
    const parsed = JSON.parse(decoder.decode(fromBase64url(token))) as { createdTs: number; id: number };
    if (Number.isFinite(parsed.createdTs) && Number.isFinite(parsed.id)) return parsed;
    return null;
  } catch {
    return null;
  }
}

function appHtml(): string {
  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Memos Worker</title>
  <style>
    :root { color-scheme: light; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f8; color: #1d252c; }
    * { box-sizing: border-box; }
    body { margin: 0; min-height: 100vh; }
    .shell { display: grid; grid-template-columns: 280px 1fr; min-height: 100vh; }
    aside { background: #ffffff; border-right: 1px solid #dde3e8; padding: 24px; position: sticky; top: 0; height: 100vh; }
    main { padding: 24px; max-width: 980px; width: 100%; }
    h1 { font-size: 22px; margin: 0 0 8px; }
    h2 { font-size: 16px; margin: 24px 0 10px; }
    label { display: block; font-size: 13px; font-weight: 650; margin: 12px 0 6px; color: #42515d; }
    input, textarea, select { width: 100%; border: 1px solid #c9d2da; border-radius: 6px; padding: 10px 11px; font: inherit; background: #fff; }
    textarea { min-height: 140px; resize: vertical; }
    button { border: 1px solid #1f6feb; background: #1f6feb; color: #fff; border-radius: 6px; padding: 9px 13px; font: inherit; font-weight: 650; cursor: pointer; }
    button.secondary { background: #fff; color: #1f2937; border-color: #c9d2da; }
    button.danger { background: #c93535; border-color: #c93535; }
    .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
    .muted { color: #687782; font-size: 13px; }
    .panel { background: #fff; border: 1px solid #dde3e8; border-radius: 8px; padding: 16px; margin-bottom: 16px; }
    .memo { background: #fff; border: 1px solid #dde3e8; border-radius: 8px; padding: 14px; margin-bottom: 12px; }
    .memo header { display: flex; justify-content: space-between; gap: 10px; color: #687782; font-size: 12px; margin-bottom: 8px; }
    .memo pre { white-space: pre-wrap; font-family: inherit; margin: 0 0 12px; line-height: 1.55; }
    .attachments { display: flex; flex-wrap: wrap; gap: 8px; margin: 0 0 12px; }
    .attachments a { color: #1f6feb; font-size: 13px; text-decoration: none; border: 1px solid #c9d2da; border-radius: 999px; padding: 4px 9px; }
    .editBox { display: grid; gap: 8px; margin: 0 0 12px; }
    .hidden { display: none; }
    .error { color: #b42318; font-size: 13px; min-height: 18px; }
    @media (max-width: 760px) { .shell { grid-template-columns: 1fr; } aside { position: static; height: auto; } main { padding: 16px; } }
  </style>
</head>
<body>
  <div class="shell">
    <aside>
      <h1>Memos Worker</h1>
      <div id="user" class="muted"></div>
      <div id="authPanel">
        <h2 id="authTitle">登录</h2>
        <label>用户名</label>
        <input id="username" autocomplete="username">
        <label>密码</label>
        <input id="password" type="password" autocomplete="current-password">
        <label class="setupOnly hidden">昵称</label>
        <input id="nickname" class="setupOnly hidden">
        <p id="authError" class="error"></p>
        <button id="authButton">登录</button>
      </div>
      <div id="sessionPanel" class="hidden">
        <h2>操作</h2>
        <div class="row">
          <button id="refreshButton" class="secondary">刷新</button>
          <button id="logoutButton" class="secondary">登出</button>
        </div>
        <h2>修改密码</h2>
        <label>当前密码</label>
        <input id="currentPassword" type="password" autocomplete="current-password">
        <label>新密码</label>
        <input id="newPassword" type="password" autocomplete="new-password">
        <p id="passwordError" class="error"></p>
        <button id="changePasswordButton" class="secondary">更新密码</button>
      </div>
    </aside>
    <main>
      <section id="composer" class="panel hidden">
        <h2>新 memo</h2>
        <textarea id="content" placeholder="记录一点什么，支持 #标签"></textarea>
        <label>可见性</label>
        <select id="visibility">
          <option value="PRIVATE">私有</option>
          <option value="PROTECTED">登录可见</option>
          <option value="PUBLIC">公开</option>
        </select>
        <label>附件</label>
        <input id="files" type="file" multiple>
        <p id="memoError" class="error"></p>
        <button id="createButton">发布</button>
      </section>
      <section>
        <div class="row" style="justify-content: space-between;">
          <h2>列表</h2>
          <input id="tagFilter" placeholder="按标签筛选" style="max-width: 180px;">
        </div>
        <div id="memos"></div>
        <button id="moreButton" class="secondary hidden">加载更多</button>
      </section>
    </main>
  </div>
  <script>
    let accessToken = localStorage.getItem("memos_access") || "";
    let setupRequired = false;
    let nextPageToken = "";

    const $ = (id) => document.getElementById(id);
    const api = async (path, options = {}) => {
      const headers = new Headers(options.headers || {});
      if (!(options.body instanceof FormData)) headers.set("Content-Type", "application/json");
      if (accessToken) headers.set("Authorization", "Bearer " + accessToken);
      const response = await fetch(path, { ...options, headers });
      const data = await response.json().catch(() => ({}));
      if (!response.ok) throw new Error(data.error || "请求失败");
      return data;
    };

    async function boot() {
      const instance = await api("/api/v1/instance");
      setupRequired = instance.setupRequired;
      $("authTitle").textContent = setupRequired ? "创建管理员" : "登录";
      $("authButton").textContent = setupRequired ? "创建并登录" : "登录";
      document.querySelectorAll(".setupOnly").forEach((node) => node.classList.toggle("hidden", !setupRequired));
      if (accessToken) await loadUser().catch(() => { accessToken = ""; localStorage.removeItem("memos_access"); });
    }

    async function loadUser() {
      const data = await api("/api/v1/auth/user");
      $("user").textContent = data.user.nickname || data.user.username;
      $("authPanel").classList.add("hidden");
      $("sessionPanel").classList.remove("hidden");
      $("composer").classList.remove("hidden");
      await loadMemos(true);
      const events = new EventSource("/api/v1/sse?access_token=" + encodeURIComponent(accessToken));
      events.addEventListener("memo.created", () => loadMemos(true));
      events.addEventListener("memo.updated", () => loadMemos(true));
      events.addEventListener("memo.deleted", () => loadMemos(true));
    }

    async function loadMemos(reset = false) {
      if (reset) nextPageToken = "";
      const params = new URLSearchParams({ page_size: "30" });
      if (nextPageToken) params.set("page_token", nextPageToken);
      if ($("tagFilter").value.trim()) params.set("tag", $("tagFilter").value.trim());
      const data = await api("/api/v1/memos?" + params);
      nextPageToken = data.nextPageToken || "";
      $("moreButton").classList.toggle("hidden", !nextPageToken);
      if (reset) $("memos").innerHTML = "";
      for (const memo of data.memos) renderMemo(memo);
    }

    function renderMemo(memo) {
      const article = document.createElement("article");
      article.className = "memo";
      article.innerHTML = '<header><span></span><span></span></header><pre></pre><div class="attachments"></div><div class="row"><button class="secondary edit">编辑</button><button class="secondary archive">归档</button></div>';
      article.querySelector("header span:first-child").textContent = memo.creator.username + " · " + new Date(memo.createdTs * 1000).toLocaleString();
      article.querySelector("header span:last-child").textContent = memo.visibility;
      article.querySelector("pre").textContent = memo.content;

      const attachments = article.querySelector(".attachments");
      for (const attachment of memo.attachments || []) {
        const link = document.createElement("a");
        link.href = attachment.url;
        link.textContent = attachment.filename;
        link.target = "_blank";
        attachments.appendChild(link);
      }
      attachments.classList.toggle("hidden", !attachments.children.length);

      article.querySelector(".edit").onclick = () => openEditor(article, memo);
      article.querySelector(".archive").onclick = async () => {
        await api("/api/v1/memos/" + encodeURIComponent(memo.uid), { method: "DELETE" });
        await loadMemos(true);
      };
      $("memos").appendChild(article);
    }

    function openEditor(article, memo) {
      const old = article.querySelector(".editBox");
      if (old) old.remove();

      const box = document.createElement("div");
      box.className = "editBox";
      box.innerHTML = '<textarea></textarea><select><option value="PRIVATE">私有</option><option value="PROTECTED">登录可见</option><option value="PUBLIC">公开</option></select><input type="file" multiple><div class="row"><button>保存</button><button class="secondary cancel">取消</button></div><p class="error"></p>';
      box.querySelector("textarea").value = memo.content;
      box.querySelector("select").value = memo.visibility;
      article.querySelector("pre").after(box);

      box.querySelector(".cancel").onclick = () => box.remove();
      box.querySelector("button").onclick = async () => {
        const error = box.querySelector(".error");
        error.textContent = "";
        try {
          await api("/api/v1/memos/" + encodeURIComponent(memo.uid), {
            method: "PATCH",
            body: JSON.stringify({
              content: box.querySelector("textarea").value,
              visibility: box.querySelector("select").value
            })
          });
          await uploadFiles(memo.uid, box.querySelector("input").files);
          await loadMemos(true);
        } catch (err) {
          error.textContent = err.message;
        }
      };
    }

    async function uploadFiles(memoUid, files) {
      for (const file of Array.from(files || [])) {
        const form = new FormData();
        form.set("file", file);
        form.set("memoUid", memoUid);
        await api("/api/v1/attachments", { method: "POST", body: form });
      }
    }

    $("authButton").onclick = async () => {
      $("authError").textContent = "";
      try {
        const path = setupRequired ? "/api/v1/setup" : "/api/v1/auth/signin";
        const data = await api(path, {
          method: "POST",
          body: JSON.stringify({
            username: $("username").value,
            password: $("password").value,
            nickname: $("nickname").value
          })
        });
        accessToken = data.accessToken;
        localStorage.setItem("memos_access", accessToken);
        await loadUser();
      } catch (error) {
        $("authError").textContent = error.message;
      }
    };

    $("createButton").onclick = async () => {
      $("memoError").textContent = "";
      try {
        const data = await api("/api/v1/memos", {
          method: "POST",
          body: JSON.stringify({ content: $("content").value, visibility: $("visibility").value })
        });
        await uploadFiles(data.memo.uid, $("files").files);
        $("content").value = "";
        $("files").value = "";
        await loadMemos(true);
      } catch (error) {
        $("memoError").textContent = error.message;
      }
    };

    $("changePasswordButton").onclick = async () => {
      $("passwordError").textContent = "";
      try {
        await api("/api/v1/auth/change-password", {
          method: "POST",
          body: JSON.stringify({
            currentPassword: $("currentPassword").value,
            newPassword: $("newPassword").value
          })
        });
        localStorage.removeItem("memos_access");
        alert("密码已更新，请重新登录");
        location.reload();
      } catch (error) {
        $("passwordError").textContent = error.message;
      }
    };

    $("refreshButton").onclick = () => loadMemos(true);
    $("moreButton").onclick = () => loadMemos(false);
    $("tagFilter").onchange = () => loadMemos(true);
    $("logoutButton").onclick = async () => {
      await api("/api/v1/auth/signout", { method: "POST", body: "{}" }).catch(() => undefined);
      localStorage.removeItem("memos_access");
      location.reload();
    };

    boot().catch((error) => { $("authError").textContent = error.message; });
  </script>
</body>
</html>`;
}
