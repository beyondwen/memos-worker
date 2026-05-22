import { pathToFileURL } from "node:url";

const DEFAULT_TIMEOUT_MS = 15_000;
export const BULK_MEMO_PATH = "/api/v1/memos/batch";

export function loadConfig(env = process.env) {
  const baseUrl = (env.MEMOS_E2E_BASE_URL || "http://127.0.0.1:8787").replace(/\/$/, "");
  const username = env.MEMOS_E2E_USERNAME || "";
  const password = env.MEMOS_E2E_PASSWORD || "";
  const keepData = env.MEMOS_E2E_KEEP_DATA === "1" || env.MEMOS_E2E_KEEP_DATA === "true";
  const timeoutMs = Number.parseInt(env.MEMOS_E2E_TIMEOUT_MS || `${DEFAULT_TIMEOUT_MS}`, 10);
  return {
    baseUrl,
    username,
    password,
    keepData,
    timeoutMs: Number.isFinite(timeoutMs) && timeoutMs > 0 ? timeoutMs : DEFAULT_TIMEOUT_MS,
  };
}

export function missingConfig(config) {
  const missing = [];
  if (!config.username) missing.push("MEMOS_E2E_USERNAME");
  if (!config.password) missing.push("MEMOS_E2E_PASSWORD");
  return missing;
}

export function authHeaders(token) {
  return {
    Authorization: `Bearer ${token}`,
    "Content-Type": "application/json",
  };
}

export async function runSmoke(config = loadConfig(), fetchImpl = globalThis.fetch) {
  const missing = missingConfig(config);
  if (missing.length > 0) {
    throw new Error(`Missing required env: ${missing.join(", ")}`);
  }
  if (typeof fetchImpl !== "function") {
    throw new Error("fetch is not available");
  }

  const client = new SmokeClient(config, fetchImpl);
  const created = [];
  let primaryMemo;
  try {
    await client.signIn();
    primaryMemo = await client.createMemo(`e2e primary ${Date.now()}`);
    created.push(primaryMemo.uid);
    const relatedMemo = await client.createMemo(`e2e related ${Date.now()}`);
    created.push(relatedMemo.uid);

    await client.createComment(primaryMemo.uid, "e2e comment");
    await client.upsertReaction(primaryMemo.uid, "👍");
    const reactions = await client.listReactions(primaryMemo.uid);
    assert(reactions.some((reaction) => reaction.reactionType === "👍"), "reaction was not persisted");

    await client.setRelations(primaryMemo.uid, relatedMemo.uid);
    const relations = await client.listRelations(primaryMemo.uid);
    assert(
      relations.some((relation) => relation.memo === `memos/${relatedMemo.uid}`),
      "relation was not persisted",
    );

    const share = await client.createShare(primaryMemo.uid);
    assert(share.uid, "share uid is missing");
    await client.fetchPublicShare(share.uid);
    await client.deleteShare(primaryMemo.uid, share.id);

    await client.bulkVisibility([primaryMemo.uid, relatedMemo.uid], "PROTECTED");
    const updated = await client.getMemo(primaryMemo.uid);
    assert(updated.visibility === "PROTECTED", "bulk visibility did not update memo");

    await client.bulkArchive([primaryMemo.uid, relatedMemo.uid]);
    const archived = await client.getMemo(primaryMemo.uid);
    assert(archived.rowStatus === "ARCHIVED", "bulk archive did not archive memo");

    return { ok: true, memoUids: created };
  } finally {
    if (!config.keepData && client.token) {
      for (const uid of created.reverse()) {
        await client.purgeMemo(uid).catch((error) => {
          console.warn(`cleanup failed for ${uid}: ${error.message}`);
        });
      }
    }
  }
}

class SmokeClient {
  constructor(config, fetchImpl) {
    this.config = config;
    this.fetch = fetchImpl;
    this.token = "";
  }

  async signIn() {
    const body = await this.request("/api/v1/auth/signin", {
      method: "POST",
      body: { username: this.config.username, password: this.config.password },
    });
    assert(body.accessToken, "signin did not return accessToken");
    this.token = body.accessToken;
  }

  async createMemo(content) {
    const body = await this.authed("/api/v1/memos", {
      method: "POST",
      body: { content, visibility: "PRIVATE" },
    });
    assert(body.memo?.uid, "create memo did not return memo uid");
    return body.memo;
  }

  async getMemo(uid) {
    const body = await this.authed(`/api/v1/memos/${encodeURIComponent(uid)}`);
    assert(body.memo?.uid === uid, "get memo returned unexpected memo");
    return body.memo;
  }

  async createComment(uid, content) {
    const body = await this.authed(`/api/v1/memos/${encodeURIComponent(uid)}/comments`, {
      method: "POST",
      body: { content },
    });
    assert(body.memo?.uid, "create comment did not return memo");
    return body.memo;
  }

  async upsertReaction(uid, reactionType) {
    return this.authed(`/api/v1/memos/${encodeURIComponent(uid)}/reactions`, {
      method: "POST",
      body: { reactionType },
    });
  }

  async listReactions(uid) {
    const body = await this.authed(`/api/v1/memos/${encodeURIComponent(uid)}/reactions`);
    assert(Array.isArray(body.reactions), "list reactions did not return reactions");
    return body.reactions;
  }

  async setRelations(uid, relatedUid) {
    return this.authed(`/api/v1/memos/${encodeURIComponent(uid)}/relations`, {
      method: "PATCH",
      body: { relations: [{ memo: `memos/${relatedUid}` }] },
    });
  }

  async listRelations(uid) {
    const body = await this.authed(`/api/v1/memos/${encodeURIComponent(uid)}/relations`);
    assert(Array.isArray(body.relations), "list relations did not return relations");
    return body.relations;
  }

  async createShare(uid) {
    const body = await this.authed(`/api/v1/memos/${encodeURIComponent(uid)}/shares`, {
      method: "POST",
      body: {},
    });
    assert(body.share?.id, "create share did not return share id");
    return body.share;
  }

  async deleteShare(uid, shareId) {
    return this.authed(`/api/v1/memos/${encodeURIComponent(uid)}/shares/${shareId}`, {
      method: "DELETE",
    });
  }

  async fetchPublicShare(shareUid) {
    const body = await this.request(`/api/v1/shares/${encodeURIComponent(shareUid)}`);
    assert(body.memo?.uid, "public share did not return memo");
    return body.memo;
  }

  async bulkVisibility(memoUids, visibility) {
    return this.authed(BULK_MEMO_PATH, {
      method: "POST",
      body: { action: "VISIBILITY", memoUids, visibility },
    });
  }

  async bulkArchive(memoUids) {
    return this.authed(BULK_MEMO_PATH, {
      method: "POST",
      body: { action: "ARCHIVE", memoUids },
    });
  }

  async purgeMemo(uid) {
    return this.authed(`/api/v1/memos/${encodeURIComponent(uid)}?purge=true`, {
      method: "DELETE",
    });
  }

  async authed(path, options = {}) {
    return this.request(path, {
      ...options,
      headers: { ...authHeaders(this.token), ...(options.headers || {}) },
    });
  }

  async request(path, options = {}) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.config.timeoutMs);
    try {
      const headers = new Headers(options.headers || {});
      let body = options.body;
      if (body && typeof body !== "string") {
        body = JSON.stringify(body);
        if (!headers.has("Content-Type")) headers.set("Content-Type", "application/json");
      }
      const response = await this.fetch(`${this.config.baseUrl}${path}`, {
        method: options.method || "GET",
        headers,
        body,
        signal: controller.signal,
      });
      const text = await response.text();
      const payload = text ? JSON.parse(text) : {};
      if (!response.ok) {
        throw new Error(`${options.method || "GET"} ${path} failed: ${response.status} ${text}`);
      }
      return payload;
    } finally {
      clearTimeout(timer);
    }
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function main() {
  const result = await runSmoke();
  console.log(JSON.stringify(result));
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
