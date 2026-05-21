import type { Env, RowStatus, Viewer, Visibility } from "../types";
import { generateUid, HttpError, json, normalizeState, readJson, unixNow } from "../utils";
import { buildMemoPayload } from "./memo";
import { recordAudit } from "./audit";

const PAGE_SIZE = 1000;
const MAX_MEMOS = 10000;

export interface OriginalMemo {
  name?: string;
  state?: string;
  creator?: string;
  createTime?: string;
  updateTime?: string;
  content?: string;
  visibility?: string;
  tags?: string[];
  pinned?: boolean;
  attachments?: Array<Record<string, unknown>>;
  relations?: Array<Record<string, unknown>>;
  reactions?: Array<Record<string, unknown>>;
  property?: Record<string, unknown>;
  parent?: string;
  snippet?: string;
  location?: Record<string, unknown>;
}

interface MigrationRequest {
  baseUrl?: string;
  accessToken?: string;
  includeArchived?: boolean;
}

export interface MigrationSummary {
  memoCount: number;
  attachmentCount: number;
  relationCount: number;
  archivedCount: number;
  truncated: boolean;
}

export interface ImportedOriginalMemo {
  uid: string;
  creatorId: number;
  content: string;
  createdTs: number;
  updatedTs: number;
  rowStatus: RowStatus;
  visibility: Visibility;
  pinned: number;
  originalName: string;
  payload: Record<string, unknown> & {
    tags: string[];
    source: {
      type: "usememos";
      originalName: string;
      creator: string;
      attachmentCount: number;
      relationCount: number;
      attachments: Array<Record<string, unknown>>;
      relations: Array<Record<string, unknown>>;
    };
  };
}

export function normalizeMemosBaseUrl(value: string): string {
  const raw = value.trim();
  if (!raw) throw new HttpError("Memos URL is required", 400);
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    throw new HttpError("Invalid Memos URL", 400);
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new HttpError("Only http and https URLs are supported", 400);
  }
  url.hash = "";
  url.search = "";
  return url.toString().replace(/\/+$/, "");
}

export function summarizeOriginalMemos(memos: OriginalMemo[], truncated = false): MigrationSummary {
  return memos.reduce<MigrationSummary>((summary, memo) => {
    summary.memoCount += 1;
    summary.attachmentCount += Array.isArray(memo.attachments) ? memo.attachments.length : 0;
    summary.relationCount += Array.isArray(memo.relations) ? memo.relations.length : 0;
    if (normalizeOriginalState(memo.state) === "ARCHIVED") summary.archivedCount += 1;
    return summary;
  }, {
    memoCount: 0,
    attachmentCount: 0,
    relationCount: 0,
    archivedCount: 0,
    truncated
  });
}

export function mapOriginalMemoToImport(memo: OriginalMemo, creatorId: number, now = unixNow()): ImportedOriginalMemo {
  const content = String(memo.content ?? "").trim();
  const originalName = String(memo.name ?? "").trim();
  const createdTs = parseTimestamp(memo.createTime, now);
  const updatedTs = parseTimestamp(memo.updateTime, createdTs);
  const attachments = Array.isArray(memo.attachments) ? memo.attachments : [];
  const relations = Array.isArray(memo.relations) ? memo.relations : [];
  const tags = Array.isArray(memo.tags) ? memo.tags.map((tag) => String(tag).trim()).filter(Boolean) : undefined;
  const payload = buildMemoPayload(content, tags) as ImportedOriginalMemo["payload"];
  payload.source = {
    type: "usememos",
    originalName,
    creator: String(memo.creator ?? ""),
    attachmentCount: attachments.length,
    relationCount: relations.length,
    attachments,
    relations
  };
  if (memo.property && typeof memo.property === "object") {
    payload.originalProperty = memo.property;
  }
  if (memo.parent) payload.originalParent = memo.parent;
  if (memo.location && typeof memo.location === "object") {
    payload.originalLocation = memo.location;
  }

  return {
    uid: generateUid("m"),
    creatorId,
    content,
    createdTs,
    updatedTs,
    rowStatus: normalizeOriginalState(memo.state),
    visibility: normalizeOriginalVisibility(memo.visibility),
    pinned: memo.pinned ? 1 : 0,
    originalName,
    payload
  };
}

export async function previewOriginalMemosMigration(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const options = await readMigrationRequest(request);
  const collected = await fetchOriginalMemos(options);
  return json({ preview: summarizeOriginalMemos(collected.memos, collected.truncated) });
}

export async function importOriginalMemos(request: Request, env: Env, viewer: Viewer): Promise<Response> {
  if (viewer.role !== "ADMIN") return json({ error: "Forbidden" }, 403);
  const options = await readMigrationRequest(request);
  const collected = await fetchOriginalMemos(options);
  const summary = summarizeOriginalMemos(collected.memos, collected.truncated);
  let imported = 0;
  let skipped = 0;

  for (const originalMemo of collected.memos) {
    const mapped = mapOriginalMemoToImport(originalMemo, viewer.id);
    if (!mapped.content) {
      skipped += 1;
      continue;
    }
    if (mapped.originalName && await hasImportedOriginalMemo(env, viewer.id, mapped.originalName)) {
      skipped += 1;
      continue;
    }

    await env.DB.prepare(`
      INSERT INTO memo (uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).bind(
      mapped.uid,
      mapped.creatorId,
      mapped.createdTs,
      mapped.updatedTs,
      mapped.rowStatus,
      mapped.content,
      mapped.visibility,
      mapped.pinned,
      JSON.stringify(mapped.payload)
    ).run();
    imported += 1;
  }

  const result = { ...summary, imported, skipped };
  await recordAudit(env, viewer, "migration.usememos.import", "usememos", {
    baseUrl: options.baseUrl,
    imported,
    skipped,
    memoCount: summary.memoCount,
    attachmentCount: summary.attachmentCount,
    relationCount: summary.relationCount,
    archivedCount: summary.archivedCount,
    truncated: summary.truncated
  });
  return json({ result });
}

async function readMigrationRequest(request: Request): Promise<Required<MigrationRequest>> {
  const body = await readJson<MigrationRequest>(request);
  const baseUrl = normalizeMemosBaseUrl(String(body.baseUrl ?? ""));
  const accessToken = String(body.accessToken ?? "").trim();
  if (!accessToken) throw new HttpError("Access token is required", 400);
  return {
    baseUrl,
    accessToken,
    includeArchived: Boolean(body.includeArchived)
  };
}

async function fetchOriginalMemos(options: Required<MigrationRequest>): Promise<{ memos: OriginalMemo[]; truncated: boolean }> {
  const states = options.includeArchived ? ["NORMAL", "ARCHIVED"] : ["NORMAL"];
  const all: OriginalMemo[] = [];
  let truncated = false;

  for (const state of states) {
    let pageToken = "";
    do {
      const url = new URL(`${options.baseUrl}/api/v1/memos`);
      url.searchParams.set("pageSize", String(PAGE_SIZE));
      url.searchParams.set("state", state);
      if (pageToken) url.searchParams.set("pageToken", pageToken);

      const response = await fetch(url.toString(), {
        headers: {
          "Accept": "application/json",
          "Authorization": `Bearer ${options.accessToken}`
        }
      });
      if (!response.ok) {
        throw new HttpError(`Original Memos API returned HTTP ${response.status}`, 400);
      }
      const data = await response.json() as { memos?: OriginalMemo[]; nextPageToken?: string };
      const memos = Array.isArray(data.memos) ? data.memos : [];
      for (const memo of memos) {
        if (all.length >= MAX_MEMOS) {
          truncated = true;
          break;
        }
        all.push(memo);
      }
      if (truncated) break;
      pageToken = String(data.nextPageToken ?? "");
    } while (pageToken);
    if (truncated) break;
  }

  return { memos: all, truncated };
}

async function hasImportedOriginalMemo(env: Env, creatorId: number, originalName: string): Promise<boolean> {
  const row = await env.DB.prepare(`
    SELECT id FROM memo
    WHERE creator_id = ?
      AND json_extract(payload, '$.source.type') = 'usememos'
      AND json_extract(payload, '$.source.originalName') = ?
    LIMIT 1
  `).bind(creatorId, originalName).first<{ id: number }>();
  return Boolean(row);
}

function parseTimestamp(value: unknown, fallback: number): number {
  if (typeof value === "number" && Number.isFinite(value)) return Math.floor(value);
  if (typeof value === "string" && value.trim()) {
    const parsed = Date.parse(value);
    if (Number.isFinite(parsed)) return Math.floor(parsed / 1000);
  }
  return fallback;
}

function normalizeOriginalState(value: unknown): RowStatus {
  const state = String(value ?? "NORMAL").toUpperCase().replace(/^STATE_/, "");
  if (!state || state === "UNSPECIFIED") return "NORMAL";
  if (state === "DELETED") return "ARCHIVED";
  return normalizeState(state || "NORMAL");
}

function normalizeOriginalVisibility(value: unknown): Visibility {
  const visibility = String(value ?? "PRIVATE").toUpperCase().replace(/^VISIBILITY_/, "");
  if (visibility === "PUBLIC" || visibility === "PROTECTED" || visibility === "PRIVATE") return visibility;
  return "PRIVATE";
}
