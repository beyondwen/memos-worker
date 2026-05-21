import type { Env, Viewer, DbMemo } from "../types";
import { json, readJson, safeJsonParse, unixNow } from "../utils";
import { buildMemoPayload } from "./memo";
import { recordAudit } from "./audit";

export function normalizeTagName(value: string): string {
  return value.trim().replace(/^#/, "").replace(/\s+/g, "-").slice(0, 64);
}

export function replaceTagInContent(content: string, from: string, to: string): string {
  const escaped = from.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return content.replace(new RegExp(`(^|\\s)#${escaped}(?=$|\\s)`, "gu"), `$1#${to}`);
}

export async function listTags(env: Env, viewer: Viewer): Promise<Response> {
  const where = viewer.role === "ADMIN" ? "" : "WHERE memo.creator_id = ?";
  const rows = await env.DB.prepare(`
    SELECT memo.id, memo.payload
    FROM memo
    ${where}
  `).bind(...(viewer.role === "ADMIN" ? [] : [viewer.id])).all<{ id: number; payload: string }>();

  const counts = new Map<string, number>();
  for (const row of rows.results) {
    const payload = safeJsonParse(row.payload, {}) as { tags?: string[] };
    for (const tag of payload.tags ?? []) counts.set(tag, (counts.get(tag) ?? 0) + 1);
  }

  return json({
    tags: [...counts.entries()]
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .map(([name, count]) => ({ name, count }))
  });
}

export async function renameTag(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  const body = await readJson<{ from?: string; to?: string }>(request);
  const from = normalizeTagName(body.from ?? "");
  const to = normalizeTagName(body.to ?? "");
  if (!from || !to) return json({ error: "from and to are required" }, 400);

  const rows = await env.DB.prepare(`
    SELECT * FROM memo WHERE content LIKE ? ${viewer.role === "ADMIN" ? "" : "AND creator_id = ?"}
  `).bind(`%#${from}%`, ...(viewer.role === "ADMIN" ? [] : [viewer.id])).all<DbMemo>();

  let updated = 0;
  for (const memo of rows.results) {
    const nextContent = replaceTagInContent(memo.content, from, to);
    if (nextContent === memo.content) continue;
    await env.DB.prepare("UPDATE memo SET content = ?, payload = ?, updated_ts = ? WHERE id = ?")
      .bind(nextContent, JSON.stringify(buildMemoPayload(nextContent)), unixNow(), memo.id)
      .run();
    updated += 1;
  }
  await recordAudit(env, viewer, "tag.rename", from, { to, updated });
  return json({ updated });
}
