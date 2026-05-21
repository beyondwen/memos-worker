import type { Env, Viewer } from "../types";
import { json } from "../utils";

export async function getTimeline(env: Env, viewer: Viewer): Promise<Response> {
  const where = ["memo.row_status = 'NORMAL'"];
  const params: unknown[] = [];
  if (viewer.role !== "ADMIN") {
    where.push("(memo.visibility != 'PRIVATE' OR memo.creator_id = ?)");
    params.push(viewer.id);
  }
  const rows = await env.DB.prepare(`
    SELECT date(memo.created_ts, 'unixepoch') AS day, COUNT(*) AS count
    FROM memo
    WHERE ${where.join(" AND ")}
    GROUP BY day
    ORDER BY day DESC
    LIMIT 120
  `).bind(...params).all<{ day: string; count: number }>();
  return json({ days: rows.results.map((row) => ({ day: row.day, count: row.count })) });
}
