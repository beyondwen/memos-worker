import { pathToFileURL } from "node:url";
import { ProxyAgent, setGlobalDispatcher } from "undici";

const DEFAULT_TIMEOUT_MS = 15_000;
export const BULK_MEMO_PATH = "/api/v1/memos/batch";

export function loadConfig(env = process.env) {
  const baseUrl = (env.MEMOS_E2E_BASE_URL || "http://127.0.0.1:8787").replace(/\/$/, "");
  const username = env.MEMOS_E2E_USERNAME || "";
  const password = env.MEMOS_E2E_PASSWORD || "";
  const webhookUrl = (env.MEMOS_E2E_WEBHOOK_URL || "").trim();
  const keepData = env.MEMOS_E2E_KEEP_DATA === "1" || env.MEMOS_E2E_KEEP_DATA === "true";
  const signup = env.MEMOS_E2E_SIGNUP === "1" || env.MEMOS_E2E_SIGNUP === "true";
  const cleanupUser = env.MEMOS_E2E_CLEANUP_USER === "1" || env.MEMOS_E2E_CLEANUP_USER === "true";
  const adminUsername = env.MEMOS_E2E_ADMIN_USERNAME || "";
  const adminPassword = env.MEMOS_E2E_ADMIN_PASSWORD || "";
  const timeoutMs = Number.parseInt(env.MEMOS_E2E_TIMEOUT_MS || `${DEFAULT_TIMEOUT_MS}`, 10);
  return {
    baseUrl,
    username,
    password,
    webhookUrl,
    keepData,
    signup,
    cleanupUser,
    adminUsername,
    adminPassword,
    proxyUrl: proxyUrlFromEnv(env),
    timeoutMs: Number.isFinite(timeoutMs) && timeoutMs > 0 ? timeoutMs : DEFAULT_TIMEOUT_MS,
  };
}

export function proxyUrlFromEnv(env = process.env) {
  return (
    env.MEMOS_E2E_PROXY_URL
    || env.HTTPS_PROXY
    || env.https_proxy
    || env.HTTP_PROXY
    || env.http_proxy
    || ""
  ).trim();
}

export function missingConfig(config) {
  const missing = [];
  if (!config.username) missing.push("MEMOS_E2E_USERNAME");
  if (!config.password) missing.push("MEMOS_E2E_PASSWORD");
  if (config.cleanupUser && !config.adminUsername) missing.push("MEMOS_E2E_ADMIN_USERNAME");
  if (config.cleanupUser && !config.adminPassword) missing.push("MEMOS_E2E_ADMIN_PASSWORD");
  return missing;
}

export function authHeaders(token) {
  return {
    Authorization: `Bearer ${token}`,
    "Content-Type": "application/json",
  };
}

export function parseSseEvents(text) {
  return text
    .split(/\n\n+/)
    .map((chunk) => {
      const event = {};
      const dataLines = [];
      for (const line of chunk.split(/\n/)) {
        if (!line || line.startsWith(":")) continue;
        const index = line.indexOf(":");
        const field = index >= 0 ? line.slice(0, index) : line;
        const value = index >= 0 ? line.slice(index + 1).trimStart() : "";
        if (field === "id") event.id = value;
        if (field === "event") event.event = value;
        if (field === "data") dataLines.push(value);
      }
      if (dataLines.length > 0) {
        event.data = JSON.parse(dataLines.join("\n"));
      }
      return event.event || event.id || event.data ? event : null;
    })
    .filter(Boolean);
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
  let webhookId = null;
  let primaryMemo;
  try {
    if (config.signup) {
      await client.signUp();
    }
    await client.signIn();
    if (config.webhookUrl) {
      webhookId = await client.createWebhook(config.webhookUrl);
    }

    primaryMemo = await client.createMemo(`e2e primary ${Date.now()}`);
    created.push(primaryMemo.uid);
    const relatedMemo = await client.createMemo(`e2e related ${Date.now()}`);
    created.push(relatedMemo.uid);

    const comment = await client.createComment(primaryMemo.uid, "e2e comment");
    created.push(comment.uid);

    await client.setRelations(primaryMemo.uid, relatedMemo.uid);
    const relations = await client.listRelations(primaryMemo.uid);
    assert(
      relations.some((relation) => relation.memo === `memos/${relatedMemo.uid}`),
      "relation was not persisted",
    );

    await client.bulkVisibility([primaryMemo.uid, relatedMemo.uid], "PROTECTED");
    const updated = await client.getMemo(primaryMemo.uid);
    assert(updated.visibility === "PROTECTED", "bulk visibility did not update memo");

    await client.bulkArchive([primaryMemo.uid, relatedMemo.uid]);
    const archived = await client.getMemo(primaryMemo.uid);
    assert(archived.rowStatus === "ARCHIVED", "bulk archive did not archive memo");

    const sseEvents = await client.fetchSseEvents();
    assertEvent(sseEvents, "memo.created", primaryMemo.uid);
    assertEvent(sseEvents, "memo.comment.created", primaryMemo.uid);
    assertEvent(sseEvents, "memo.bulk.updated", primaryMemo.uid);

    let webhookDeliveries = [];
    if (webhookId) {
      webhookDeliveries = await client.listWebhookDeliveries(webhookId);
      assert(
        webhookDeliveries.some((delivery) => delivery.event === "memo.created"),
        "webhook delivery for memo.created was not found",
      );
    }

    return {
      ok: true,
      memoUids: created,
      sseEvents: sseEvents.map((event) => event.event).filter(Boolean),
      webhookDeliveryCount: webhookDeliveries.length,
    };
  } finally {
    if (!config.keepData && client.token) {
      if (webhookId) {
        await client.deleteWebhook(webhookId).catch((error) => {
          console.warn(`cleanup webhook failed for ${webhookId}: ${error.message}`);
        });
      }
      for (const uid of created.reverse()) {
        await client.purgeMemo(uid).catch((error) => {
          console.warn(`cleanup failed for ${uid}: ${error.message}`);
        });
      }
      if (config.cleanupUser) {
        await cleanupUser(config, fetchImpl).catch((error) => {
          console.warn(`cleanup user failed for ${config.username}: ${error.message}`);
        });
      }
    }
  }
}

export function configureProxy(config = loadConfig()) {
  if (!config.proxyUrl) return false;
  setGlobalDispatcher(new ProxyAgent(config.proxyUrl));
  return true;
}

async function cleanupUser(config, fetchImpl) {
  const adminClient = new SmokeClient(
    {
      ...config,
      username: config.adminUsername,
      password: config.adminPassword,
    },
    fetchImpl,
  );
  await adminClient.signIn();
  await adminClient.deleteUser(config.username);
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

  async signUp() {
    return this.request("/api/v1/auth/signup", {
      method: "POST",
      body: {
        username: this.config.username,
        password: this.config.password,
        nickname: `E2E ${this.config.username}`,
      },
    });
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

  async deleteUser(username) {
    return this.authed(`/api/v1/users/${encodeURIComponent(username)}`, { method: "DELETE" });
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

  async fetchSseEvents() {
    const body = await this.request(`/api/v1/sse?access_token=${encodeURIComponent(this.token)}`, {
      headers: { Authorization: `Bearer ${this.token}` },
    }, "text");
    return parseSseEvents(body);
  }

  async createWebhook(url) {
    const body = await this.authed("/api/v1/webhooks", {
      method: "POST",
      body: { name: `e2e-${Date.now()}`, url },
    });
    assert(body.webhook?.id, "create webhook did not return id");
    return body.webhook.id;
  }

  async deleteWebhook(id) {
    return this.authed(`/api/v1/webhooks/${id}`, { method: "DELETE" });
  }

  async listWebhookDeliveries(webhookId) {
    const body = await this.authed(`/api/v1/webhooks/deliveries?webhookId=${webhookId}`);
    assert(Array.isArray(body.deliveries), "webhook deliveries response did not return deliveries");
    return body.deliveries;
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

  async request(path, options = {}, responseType = "json") {
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
      if (!response.ok) {
        throw new Error(`${options.method || "GET"} ${path} failed: ${response.status} ${text}`);
      }
      if (responseType === "text") return text;
      const payload = text ? JSON.parse(text) : {};
      return payload;
    } finally {
      clearTimeout(timer);
    }
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function assertEvent(events, eventType, memoUid) {
  assert(
    events.some((event) => event.event === eventType && event.data?.name === `memos/${memoUid}`),
    `SSE event ${eventType} for ${memoUid} was not found`,
  );
}

async function main() {
  const config = loadConfig();
  configureProxy(config);
  const result = await runSmoke(config);
  console.log(JSON.stringify(result));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
