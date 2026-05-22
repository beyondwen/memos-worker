import { describe, expect, it } from "vitest";

import {
  BULK_MEMO_PATH,
  SmokeClient,
  authHeaders,
  loadConfig,
  missingConfig,
  parseSseEvents,
  proxyUrlFromEnv,
} from "../scripts/e2e-smoke.js";

describe("e2e smoke script helpers", () => {
  it("loads defaults and credentials from env", () => {
    expect(loadConfig({
      MEMOS_E2E_USERNAME: "alice",
      MEMOS_E2E_PASSWORD: "secret",
    })).toEqual({
      baseUrl: "http://127.0.0.1:8787",
      username: "alice",
      password: "secret",
      keepData: false,
      signup: false,
      security: false,
      maintenance: false,
      cleanupUser: false,
      adminUsername: "",
      adminPassword: "",
      timeoutMs: 15_000,
      proxyUrl: "",
    });
  });

  it("normalizes base url and keep-data flag", () => {
    expect(loadConfig({
      MEMOS_E2E_BASE_URL: "https://example.test/",
      MEMOS_E2E_USERNAME: "alice",
      MEMOS_E2E_PASSWORD: "secret",
      MEMOS_E2E_KEEP_DATA: "1",
      MEMOS_E2E_TIMEOUT_MS: "5000",
      MEMOS_E2E_PROXY_URL: "http://127.0.0.1:7890",
    })).toMatchObject({
      baseUrl: "https://example.test",
      keepData: true,
      timeoutMs: 5000,
      proxyUrl: "http://127.0.0.1:7890",
    });
  });

  it("loads signup and cleanup account options", () => {
    expect(loadConfig({
      MEMOS_E2E_USERNAME: "e2e_user",
      MEMOS_E2E_PASSWORD: "secret123",
      MEMOS_E2E_SIGNUP: "1",
      MEMOS_E2E_SECURITY: "true",
      MEMOS_E2E_MAINTENANCE: "1",
      MEMOS_E2E_CLEANUP_USER: "true",
      MEMOS_E2E_ADMIN_USERNAME: "admin",
      MEMOS_E2E_ADMIN_PASSWORD: "admin-secret",
    })).toMatchObject({
      signup: true,
      security: true,
      maintenance: true,
      cleanupUser: true,
      adminUsername: "admin",
      adminPassword: "admin-secret",
    });
  });

  it("reports missing credentials explicitly", () => {
    expect(missingConfig(loadConfig({}))).toEqual([
      "MEMOS_E2E_USERNAME",
      "MEMOS_E2E_PASSWORD",
    ]);
  });

  it("requires admin credentials when cleanup user is enabled", () => {
    expect(missingConfig(loadConfig({
      MEMOS_E2E_USERNAME: "e2e_user",
      MEMOS_E2E_PASSWORD: "secret123",
      MEMOS_E2E_CLEANUP_USER: "1",
    }))).toEqual([
      "MEMOS_E2E_ADMIN_USERNAME",
      "MEMOS_E2E_ADMIN_PASSWORD",
    ]);
  });

  it("prefers explicit e2e proxy over process proxy env", () => {
    expect(proxyUrlFromEnv({
      MEMOS_E2E_PROXY_URL: "http://127.0.0.1:7890",
      HTTPS_PROXY: "http://127.0.0.1:7891",
    })).toBe("http://127.0.0.1:7890");
    expect(proxyUrlFromEnv({
      HTTPS_PROXY: "http://127.0.0.1:7891",
    })).toBe("http://127.0.0.1:7891");
  });

  it("builds bearer auth headers", () => {
    expect(authHeaders("token")).toEqual({
      Authorization: "Bearer token",
      "Content-Type": "application/json",
    });
  });

  it("uses the Rust bulk memo route", () => {
    expect(BULK_MEMO_PATH).toBe("/api/v1/memos/batch");
  });

  it("parses SSE event chunks with ids and JSON payloads", () => {
    expect(parseSseEvents([
      "retry: 5000",
      "event: ready",
      "data: {\"userId\":1}",
      "",
      "id: 42",
      "event: memo.created",
      "data: {\"name\":\"memos/m1\"}",
      "",
    ].join("\n"))).toEqual([
      { event: "ready", data: { userId: 1 } },
      { id: "42", event: "memo.created", data: { name: "memos/m1" } },
    ]);
  });

  it("security smoke checks enforce signup, csrf and revoked token expectations", async () => {
    const calls = [];
    const fetchImpl = async (url, options = {}) => {
      const path = new URL(url).pathname;
      calls.push({ path, options });
      if (path === "/api/v1/auth/signup") {
        return jsonResponse({ error: "Public signup is disabled" }, 403);
      }
      if (path === "/api/v1/memos" && options.headers?.get?.("Cookie") && !options.headers?.get?.("Authorization")) {
        return jsonResponse({ error: "Invalid CSRF token" }, 403);
      }
      if (path === "/api/v1/memos" && options.headers?.get?.("Authorization")) {
        return jsonResponse({ memo: { uid: "m_security" } }, 201);
      }
      if (path === "/api/v1/memos/m_security") {
        return jsonResponse({ ok: true }, 200);
      }
      if (path === "/api/v1/auth/signout") {
        return jsonResponse({ ok: true }, 200);
      }
      if (path === "/api/v1/auth/user") {
        return jsonResponse({ error: "Unauthorized" }, 401);
      }
      if (path === "/api/v1/auth/signin") {
        return new Response(JSON.stringify({ accessToken: "token" }), {
          status: 200,
          headers: {
            "Content-Type": "application/json",
            "Set-Cookie": "memos_csrf=csrf; Path=/, memos_access=access; Path=/api/v1",
          },
        });
      }
      return jsonResponse({}, 200);
    };
    const client = new SmokeClient(loadConfig({
      MEMOS_E2E_USERNAME: "alice",
      MEMOS_E2E_PASSWORD: "secret123",
      MEMOS_E2E_SECURITY: "1",
    }), fetchImpl);
    await client.signIn();
    client.cookies.set("memos_csrf", "csrf");
    client.cookies.set("memos_access", "access");

    await client.assertSecurityDefaults();

    expect(calls.map((call) => call.path)).toContain("/api/v1/auth/signup");
    expect(calls.map((call) => call.path)).toContain("/api/v1/auth/user");
  });

  it("maintenance smoke covers health, index rebuild and backup preview", async () => {
    const calls = [];
    const fetchImpl = async (url) => {
      const path = new URL(url).pathname;
      calls.push(path);
      if (path === "/api/v1/auth/signin") {
        return jsonResponse({ accessToken: "token" }, 200);
      }
      if (path === "/api/v1/system/health") {
        return jsonResponse({
          status: "healthy",
          memoIndex: { memoCount: 1, healthy: true },
          backup: { count: 1 },
        }, 200);
      }
      if (path === "/api/v1/memo-index/rebuild") {
        return jsonResponse({ rebuilt: 1, memoIndex: { healthy: true } }, 200);
      }
      if (path === "/api/v1/backups") {
        return jsonResponse({ backup: { key: "backups/e2e.json" } }, 201);
      }
      if (path === "/api/v1/backups/preview") {
        return jsonResponse({ preview: { memoCount: 1 } }, 200);
      }
      return jsonResponse({}, 200);
    };
    const client = new SmokeClient(loadConfig({
      MEMOS_E2E_USERNAME: "admin",
      MEMOS_E2E_PASSWORD: "secret123",
      MEMOS_E2E_MAINTENANCE: "1",
    }), fetchImpl);
    await client.signIn();

    await client.assertMaintenanceDefaults();

    expect(calls).toContain("/api/v1/system/health");
    expect(calls).toContain("/api/v1/memo-index/rebuild");
    expect(calls).toContain("/api/v1/backups/preview");
  });
});

function jsonResponse(body, status) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
