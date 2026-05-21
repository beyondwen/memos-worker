import type { Env, Viewer, DbAttachment } from "../types";
import { json, unixNow, generateUid, sanitizeFilename } from "../utils";
import { canWriteMemo, getMemoByUid } from "./memo";

export function publicAttachment(attachment: DbAttachment): Record<string, unknown> {
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

export function canReadAttachment(attachment: DbAttachment, viewer: Viewer): boolean {
  if (viewer.role === "ADMIN") return true;
  if (!attachment.memo_id) return attachment.creator_id === viewer.id;
  if (attachment.memo_visibility !== "PRIVATE") return true;
  return attachment.memo_creator_id === viewer.id;
}

export async function getAttachmentById(env: Env, id: number): Promise<DbAttachment | null> {
  return await env.DB.prepare(`
    SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id
    FROM attachment
    LEFT JOIN memo ON memo.id = attachment.memo_id
    WHERE attachment.id = ?
  `).bind(id).first<DbAttachment>();
}

export async function getAttachmentByUid(env: Env, uid: string): Promise<DbAttachment | null> {
  return await env.DB.prepare(`
    SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id
    FROM attachment
    LEFT JOIN memo ON memo.id = attachment.memo_id
    WHERE attachment.uid = ?
  `).bind(uid).first<DbAttachment>();
}

export async function uploadAttachment(request: Request, env: Env, viewer: Viewer): Promise<Response> {
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

export async function downloadAttachment(env: Env, viewer: Viewer, uid: string): Promise<Response> {
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

export async function listAttachments(env: Env, viewer: Viewer): Promise<Response> {
  const where: string[] = [];
  const params: unknown[] = [];

  if (viewer.role !== "ADMIN") {
    where.push("attachment.creator_id = ?");
    params.push(viewer.id);
  }

  const sql = `
    SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id
    FROM attachment
    LEFT JOIN memo ON memo.id = attachment.memo_id
    ${where.length ? "WHERE " + where.join(" AND ") : ""}
    ORDER BY attachment.created_ts DESC
    LIMIT 100
  `;

  const rows = await env.DB.prepare(sql).bind(...params).all<DbAttachment>();
  return json({ attachments: rows.results.map(publicAttachment) });
}
