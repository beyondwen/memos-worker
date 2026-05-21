import type { DbMemo, Env, Viewer } from "../types";
import { HttpError, json, safeJsonParse } from "../utils";
import { canReadMemo, getMemoByUid } from "./memo";
import { resolveAiRuntimeSettings } from "./aiSettings";

const RECENT_CANDIDATE_LIMIT = 80;
const AI_CANDIDATE_LIMIT = 30;
const SUGGESTION_LIMIT = 8;

export interface RelationCandidate {
  uid: string;
  content: string;
  payload: string;
  updated_ts: number;
}

export interface RankedRelationCandidate {
  uid: string;
  content: string;
  score: number;
  tags: string[];
}

export interface AiRelationSuggestion {
  memo: string;
  content: string;
  reason: string;
  confidence: number;
  source: "ai" | "local";
}

export function rankRelationCandidates(
  current: RelationCandidate,
  candidates: RelationCandidate[],
  limit = AI_CANDIDATE_LIMIT
): RankedRelationCandidate[] {
  const currentTags = extractPayloadTags(current.payload);
  const currentKeywords = extractKeywords(current.content);
  const maxUpdated = Math.max(...candidates.map((candidate) => candidate.updated_ts), current.updated_ts, 1);

  return candidates
    .filter((candidate) => candidate.uid !== current.uid && candidate.content.trim())
    .map((candidate) => {
      const tags = extractPayloadTags(candidate.payload);
      const keywords = extractKeywords(candidate.content);
      const sharedTags = tags.filter((tag) => currentTags.includes(tag)).length;
      const sharedKeywords = keywords.filter((keyword) => currentKeywords.includes(keyword)).length;
      const recency = Math.max(0, Math.min(1, candidate.updated_ts / maxUpdated));
      return {
        uid: candidate.uid,
        content: candidate.content,
        score: sharedTags * 5 + sharedKeywords * 2 + recency,
        tags
      };
    })
    .filter((candidate) => candidate.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit);
}

export function parseAiRelationSuggestions(
  raw: string,
  candidatesByUid: Map<string, Pick<RelationCandidate, "uid" | "content">>
): AiRelationSuggestion[] {
  const parsed = safeJsonParse<Record<string, unknown>>(raw, {});
  const list = Array.isArray(parsed.suggestions) ? parsed.suggestions : [];
  const seen = new Set<string>();
  const suggestions: AiRelationSuggestion[] = [];

  for (const item of list) {
    if (!item || typeof item !== "object") continue;
    const record = item as Record<string, unknown>;
    const uid = String(record.memo ?? "").replace(/^memos\//, "").trim();
    const candidate = candidatesByUid.get(uid);
    if (!uid || !candidate || seen.has(uid)) continue;
    seen.add(uid);
    suggestions.push({
      memo: `memos/${uid}`,
      content: candidate.content,
      reason: String(record.reason ?? "内容相关").slice(0, 160),
      confidence: clampConfidence(Number(record.confidence ?? 0.5)),
      source: "ai"
    });
    if (suggestions.length >= SUGGESTION_LIMIT) break;
  }

  return suggestions;
}

export async function suggestMemoRelations(env: Env, viewer: Viewer, memoUid: string): Promise<Response> {
  const memo = await getMemoByUid(env, memoUid);
  if (!memo) return json({ error: "Memo not found" }, 404);
  if (!canReadMemo(memo, viewer)) return json({ error: "Forbidden" }, 403);

  const rows = await env.DB.prepare(`
    SELECT memo.uid, memo.content, memo.payload, memo.updated_ts
    FROM memo
    WHERE memo.row_status = 'NORMAL'
      AND memo.id != ?
      AND (memo.visibility != 'PRIVATE' OR memo.creator_id = ? OR ? = 'ADMIN')
      AND NOT EXISTS (
        SELECT 1 FROM memo_relation
        WHERE memo_relation.memo_id = ?
          AND memo_relation.related_memo_id = memo.id
          AND memo_relation.type = 'REFERENCE'
      )
    ORDER BY memo.updated_ts DESC, memo.id DESC
    LIMIT ?
  `).bind(memo.id, viewer.id, viewer.role, memo.id, RECENT_CANDIDATE_LIMIT).all<RelationCandidate>();

  const ranked = rankRelationCandidates(toRelationCandidate(memo), rows.results, AI_CANDIDATE_LIMIT);
  if (ranked.length === 0) return json({ suggestions: [] });

  const candidateMap = new Map(ranked.map((candidate) => [
    candidate.uid,
    { uid: candidate.uid, content: candidate.content }
  ]));

  const aiSettings = await resolveAiRuntimeSettings(env);
  const aiSuggestions = aiSettings.apiKey
    ? await requestAiSuggestions(aiSettings, memo, ranked, candidateMap).catch(() => [])
    : [];

  const suggestions = aiSuggestions.length > 0
    ? aiSuggestions
    : ranked.slice(0, 5).map((candidate) => ({
      memo: `memos/${candidate.uid}`,
      content: candidate.content,
      reason: "标签或关键词相近",
      confidence: Math.min(0.75, Math.max(0.35, candidate.score / 10)),
      source: "local" as const
    }));

  return json({ suggestions });
}

async function requestAiSuggestions(
  settings: { baseUrl: string; model: string; apiKey: string },
  memo: DbMemo,
  candidates: RankedRelationCandidate[],
  candidateMap: Map<string, Pick<RelationCandidate, "uid" | "content">>
): Promise<AiRelationSuggestion[]> {
  const response = await fetch(`${settings.baseUrl.replace(/\/+$/, "")}/chat/completions`, {
    method: "POST",
    headers: {
      "Authorization": `Bearer ${settings.apiKey}`,
      "Content-Type": "application/json"
    },
    body: JSON.stringify({
      model: settings.model,
      temperature: 0.1,
      response_format: { type: "json_object" },
      messages: [
        {
          role: "system",
          content: "你是个人知识库的关联识别助手。只返回 JSON，不要解释。"
        },
        {
          role: "user",
          content: JSON.stringify({
            task: "从 candidates 中选择最多 8 条和 currentMemo 最相关的笔记。返回 {\"suggestions\":[{\"memo\":\"memos/<uid>\",\"reason\":\"简短原因\",\"confidence\":0.0到1.0}]}。",
            currentMemo: {
              memo: `memos/${memo.uid}`,
              content: memo.content.slice(0, 1200)
            },
            candidates: candidates.map((candidate) => ({
              memo: `memos/${candidate.uid}`,
              content: candidate.content.slice(0, 600),
              tags: candidate.tags
            }))
          })
        }
      ]
    })
  });
  if (!response.ok) throw new HttpError(`AI API returned HTTP ${response.status}`, 502);
  const data = await response.json() as { choices?: Array<{ message?: { content?: string } }> };
  return parseAiRelationSuggestions(String(data.choices?.[0]?.message?.content ?? ""), candidateMap);
}

function toRelationCandidate(memo: DbMemo): RelationCandidate {
  return {
    uid: memo.uid,
    content: memo.content,
    payload: memo.payload,
    updated_ts: memo.updated_ts
  };
}

function extractPayloadTags(payload: string): string[] {
  const parsed = safeJsonParse<{ tags?: unknown[] }>(payload, {});
  return Array.isArray(parsed.tags)
    ? parsed.tags.map((tag) => String(tag).trim()).filter(Boolean)
    : [];
}

function extractKeywords(content: string): string[] {
  const words = new Set<string>();
  for (const match of content.toLowerCase().matchAll(/[\p{L}\p{N}_-]{2,}/gu)) {
    const word = match[0];
    if (word.length > 32) continue;
    words.add(word);
  }
  return [...words].slice(0, 80);
}

function clampConfidence(value: number): number {
  if (!Number.isFinite(value)) return 0.5;
  return Math.max(0, Math.min(1, value));
}
