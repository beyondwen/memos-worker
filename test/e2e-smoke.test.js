import { describe, expect, it } from "vitest";

import {
  BULK_MEMO_PATH,
  authHeaders,
  loadConfig,
  missingConfig,
  parseSseEvents,
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
      timeoutMs: 15_000,
      webhookUrl: "",
    });
  });

  it("normalizes base url and keep-data flag", () => {
    expect(loadConfig({
      MEMOS_E2E_BASE_URL: "https://example.test/",
      MEMOS_E2E_USERNAME: "alice",
      MEMOS_E2E_PASSWORD: "secret",
      MEMOS_E2E_KEEP_DATA: "1",
      MEMOS_E2E_TIMEOUT_MS: "5000",
      MEMOS_E2E_WEBHOOK_URL: "https://example.test/hook",
    })).toMatchObject({
      baseUrl: "https://example.test",
      keepData: true,
      timeoutMs: 5000,
      webhookUrl: "https://example.test/hook",
    });
  });

  it("reports missing credentials explicitly", () => {
    expect(missingConfig(loadConfig({}))).toEqual([
      "MEMOS_E2E_USERNAME",
      "MEMOS_E2E_PASSWORD",
    ]);
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
