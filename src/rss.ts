import type { Env, DbMemo } from "./types";
import { getUserById } from "./middleware";

export async function generateRss(env: Env, username?: string): Promise<Response> {
  let where = "memo.row_status = 'NORMAL' AND memo.visibility = 'PUBLIC'";
  const params: unknown[] = [];

  if (username) {
    where += ' AND "user".username = ?';
    params.push(username);
  }

  const rows = await env.DB.prepare(`
    SELECT memo.*, "user".username AS creator_username, "user".nickname AS creator_nickname
    FROM memo
    JOIN "user" ON "user".id = memo.creator_id
    WHERE ${where}
    ORDER BY memo.created_ts DESC
    LIMIT 50
  `).bind(...params).all<DbMemo>();

  let siteName = "Memos Worker";
  let siteDescription = "A lightweight, self-hosted memo hub";
  const instanceName = await env.DB.prepare("SELECT value FROM system_setting WHERE name = 'site_title'").first<{ value: string }>();
  if (instanceName) siteName = instanceName.value;

  const items = rows.results.map(memo => {
    const pubDate = new Date(memo.created_ts * 1000).toUTCString();
    const creator = memo.creator_username || "unknown";
    const escapedContent = escapeXml(memo.content);
    const title = extractTitle(memo.content) || `Memo by ${creator}`;

    return `    <item>
      <title>${escapeXml(title)}</title>
      <link>/memos/${memo.uid}</link>
      <guid isPermaLink="false">memos/${memo.uid}</guid>
      <pubDate>${pubDate}</pubDate>
      <author>${escapeXml(creator)}</author>
      <description>${escapedContent}</description>
    </item>`;
  }).join("\n");

  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>${escapeXml(siteName)}</title>
    <link>/</link>
    <description>${escapeXml(siteDescription)}</description>
    <lastBuildDate>${new Date().toUTCString()}</lastBuildDate>
    <atom:link href="/api/v1${username ? `/u/${username}` : "/explore"}/rss.xml" rel="self" type="application/rss+xml"/>
${items}
  </channel>
</rss>`;

  return new Response(xml, {
    headers: {
      "Content-Type": "application/rss+xml; charset=utf-8",
      "Cache-Control": "public, max-age=600"
    }
  });
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function extractTitle(content: string): string {
  const firstLine = content.split("\n")[0]?.trim() ?? "";
  const cleaned = firstLine.replace(/^#+\s*/, "").replace(/[*_`~]/g, "");
  return cleaned.length > 120 ? cleaned.slice(0, 120) + "…" : cleaned;
}
