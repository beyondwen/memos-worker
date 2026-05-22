import { describe, expect, it } from "vitest";

import {
  BULK_MEMO_PATH,
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
      MEMOS_E2E_CLEANUP_USER: "true",
      MEMOS_E2E_ADMIN_USERNAME: "admin",
      MEMOS_E2E_ADMIN_PASSWORD: "admin-secret",
    })).toMatchObject({
      signup: true,
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
});
