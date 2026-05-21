import { describe, expect, it } from "vitest";
import { buildMemoListPath } from "../web/src/memoQuery";
import { buildAdvancedMemoFilter } from "../web/src/advancedSearch";
import { mergeRelationInputWithSuggestions, parseRelationInput } from "../web/src/relationView";
import {
  clearEditorDraft,
  loadEditorDraft,
  saveEditorDraft,
  type StorageLike,
} from "../web/src/editorDraft";
import { attachmentDisplayMeta } from "../web/src/attachmentView";
import { shouldRefreshForSseEvent } from "../web/src/sseEvents";
import { buildAuthRedirectPath } from "../web/src/authFlow";
import { formatInboxItem } from "../web/src/inboxView";
import { buildShareUrl, normalizeWebhookForm } from "../web/src/integrationHelpers";
import { buildBulkMemoRequest, bulkMemoActionLabel } from "../web/src/bulkActions";
import { MEMO_WEBHOOK_EVENTS } from "../src/webhookEvents";
import { webhookDeliveryStatusMeta, webhookDeliveryTimeLabel } from "../web/src/webhookDeliveryView";
import { highlightRenderedHtml } from "../web/src/searchHighlight";
import { applyMemoTemplate, MEMO_TEMPLATES } from "../web/src/memoTemplates";
import { buildSearchSnippet, scoreSearchMatch } from "../web/src/searchResultView";
import { attachmentCleanupSummary } from "../web/src/attachmentCleanupView";
import { buildHomeDateFilterPath, parseHomeDateFilterParams, stripHomeFilterParams } from "../web/src/homeFilters";
import { shouldOpenMemoDetailFromCardClick } from "../web/src/cardClick";

class MemoryStorage implements StorageLike {
  private values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }
}

describe("memo list query builder", () => {
  it("builds backend-compatible query params for search and filters", () => {
    const path = buildMemoListPath({
      tag: "work",
      visibility: "PUBLIC",
      state: "ARCHIVED",
      search: 'hello "memo"',
      propertyFilter: "has_code",
      pageToken: "cursor-1",
      pageSize: 30,
    });

    const url = new URL(path, "https://example.test");
    expect(url.pathname).toBe("/api/v1/memos");
    expect(url.searchParams.get("tag")).toBe("work");
    expect(url.searchParams.get("visibility")).toBe("PUBLIC");
    expect(url.searchParams.get("state")).toBe("ARCHIVED");
    expect(url.searchParams.get("page_token")).toBe("cursor-1");
    expect(url.searchParams.get("page_size")).toBe("30");
    expect(url.searchParams.get("filter")).toBe('content.contains("hello \\"memo\\"") && has_code');
  });

  it("omits empty optional filters", () => {
    const path = buildMemoListPath({ search: "   ", pageSize: 20 });

    const url = new URL(path, "https://example.test");
    expect(url.searchParams.has("filter")).toBe(false);
    expect(url.searchParams.get("page_size")).toBe("20");
  });
});

describe("search highlighting", () => {
  it("highlights matching text without touching html tags", () => {
    expect(highlightRenderedHtml("<p>Hello <strong>memo</strong></p>", "memo")).toBe(
      '<p>Hello <strong><mark class="search-hit">memo</mark></strong></p>'
    );
  });

  it("ignores blank search terms", () => {
    expect(highlightRenderedHtml("<p>Hello memo</p>", "  ")).toBe("<p>Hello memo</p>");
  });
});

describe("search result view helpers", () => {
  it("scores tag matches higher than plain content matches", () => {
    expect(scoreSearchMatch({ content: "hello", tags: ["work"] }, "work")).toBeGreaterThan(
      scoreSearchMatch({ content: "work item", tags: [] }, "work")
    );
  });

  it("builds a short snippet around the match", () => {
    expect(buildSearchSnippet("alpha beta gamma delta", "gamma", 8)).toBe("...eta gamma del...");
  });
});

describe("attachment cleanup view helpers", () => {
  it("summarizes attachment count and size", () => {
    expect(attachmentCleanupSummary([
      { size: 1024 },
      { size: 2048 },
    ])).toEqual({ count: 2, size: 3072, sizeLabel: "3.0 KB" });
  });
});

describe("memo templates", () => {
  it("exposes practical quick memo templates", () => {
    expect(MEMO_TEMPLATES.map((template) => template.id)).toEqual([
      "todo",
      "meeting",
      "study",
      "bug",
      "daily",
    ]);
  });

  it("appends a template after existing content", () => {
    expect(applyMemoTemplate("已有内容", "todo")).toContain("已有内容\n\n## TODO");
  });
});

describe("relation view helpers", () => {
  it("merges AI suggestions into existing relation input without duplicates", () => {
    expect(mergeRelationInputWithSuggestions("m_a", [
      { memo: "memos/m_a", content: "A", reason: "duplicate", confidence: 0.9, source: "ai" },
      { memo: "memos/m_b", content: "B", reason: "related", confidence: 0.8, source: "ai" },
    ])).toBe("m_a\nm_b");
  });
});

describe("advanced memo filters", () => {
  it("combines creator, pinned and date range filters", () => {
    expect(buildAdvancedMemoFilter({
      creator: "admin",
      pinned: "PINNED",
      createdAfter: "2026-05-01",
      createdBefore: "2026-05-21",
    })).toBe('creator == "admin" && pinned == true && created_ts >= 1777593600 && created_ts <= 1779321600');
  });

  it("omits empty advanced filters", () => {
    expect(buildAdvancedMemoFilter({ creator: "  ", pinned: "" })).toBe("");
  });
});

describe("home URL filters", () => {
  it("parses timeline date filters from the URL query", () => {
    expect(parseHomeDateFilterParams("?createdAfter=2026-05-21&createdBefore=2026-05-21")).toEqual({
      createdAfter: "2026-05-21",
      createdBefore: "2026-05-21",
    });
  });

  it("builds and clears date-filtered home paths without stale query params", () => {
    expect(buildHomeDateFilterPath("2026-05-21")).toBe("/?createdAfter=2026-05-21&createdBefore=2026-05-21");
    expect(stripHomeFilterParams("/?createdAfter=2026-05-21&createdBefore=2026-05-21&foo=bar")).toBe("/?foo=bar");
    expect(stripHomeFilterParams("/?createdAfter=2026-05-21&createdBefore=2026-05-21")).toBe("/");
  });
});

describe("relation input parser", () => {
  it("parses memo refs from mixed input", () => {
    expect(parseRelationInput("memos/a1\nb2, https://example.test/#/memos/c3")).toEqual([
      { memo: "memos/a1", type: "REFERENCE" },
      { memo: "memos/b2", type: "REFERENCE" },
      { memo: "memos/c3", type: "REFERENCE" },
    ]);
  });

  it("deduplicates and ignores blank relation refs", () => {
    expect(parseRelationInput("a1, memos/a1,  ")).toEqual([
      { memo: "memos/a1", type: "REFERENCE" },
    ]);
  });
});

describe("editor draft storage", () => {
  it("saves and restores a valid draft", () => {
    const storage = new MemoryStorage();

    saveEditorDraft(storage, {
      content: "hello draft",
      visibility: "PROTECTED",
      attachmentUids: ["a1"],
    }, 123);

    expect(loadEditorDraft(storage)).toEqual({
      content: "hello draft",
      visibility: "PROTECTED",
      attachmentUids: ["a1"],
      savedAt: 123,
    });
  });

  it("clears the draft when content and attachments are empty", () => {
    const storage = new MemoryStorage();

    saveEditorDraft(storage, {
      content: "hello",
      visibility: "PRIVATE",
      attachmentUids: [],
    }, 123);
    saveEditorDraft(storage, {
      content: "   ",
      visibility: "PRIVATE",
      attachmentUids: [],
    }, 124);

    expect(loadEditorDraft(storage)).toBeNull();
  });

  it("ignores invalid stored payloads", () => {
    const storage = new MemoryStorage();
    storage.setItem("memos_editor_draft_v1", "{bad json");

    expect(loadEditorDraft(storage)).toBeNull();
  });

  it("clears the saved draft explicitly", () => {
    const storage = new MemoryStorage();
    saveEditorDraft(storage, {
      content: "hello",
      visibility: "PUBLIC",
      attachmentUids: [],
    }, 123);

    clearEditorDraft(storage);

    expect(loadEditorDraft(storage)).toBeNull();
  });
});

describe("attachment display metadata", () => {
  it("marks image attachments as previewable and formats their size", () => {
    expect(attachmentDisplayMeta({ filename: "photo.png", type: "image/png", size: 1536 })).toEqual({
      icon: "IMG",
      isImage: true,
      sizeLabel: "1.5 KB",
      typeLabel: "PNG",
    });
  });

  it("falls back to file extension when content type is generic", () => {
    expect(attachmentDisplayMeta({ filename: "report.pdf", type: "application/octet-stream", size: 2_097_152 })).toEqual({
      icon: "PDF",
      isImage: false,
      sizeLabel: "2 MB",
      typeLabel: "PDF",
    });
  });
});

describe("SSE refresh policy", () => {
  it("refreshes memo lists for memo and relation changes", () => {
    expect(shouldRefreshForSseEvent({ type: "memo.created", name: "memos/a" })).toBe(true);
    expect(shouldRefreshForSseEvent({ type: "memo.archived", name: "memos/a" })).toBe(true);
    expect(shouldRefreshForSseEvent({ type: "memo.restored", name: "memos/a" })).toBe(true);
    expect(shouldRefreshForSseEvent({ type: "memo.bulk.updated", name: "memos/a" })).toBe(true);
    expect(shouldRefreshForSseEvent({ type: "memo.comment.created", name: "memos/a" })).toBe(true);
    expect(shouldRefreshForSseEvent({ type: "reaction.upserted", name: "memos/a" })).toBe(true);
  });

  it("ignores malformed events", () => {
    expect(shouldRefreshForSseEvent(null)).toBe(false);
    expect(shouldRefreshForSseEvent({ type: "ready", name: "memos/a" })).toBe(false);
    expect(shouldRefreshForSseEvent({ type: "memo.created" })).toBe(false);
  });
});

describe("bulk memo helpers", () => {
  it("builds archive requests with trimmed unique memo UIDs", () => {
    expect(buildBulkMemoRequest("ARCHIVE", [" m1 ", "m2", "m1", ""])).toEqual({
      ok: true,
      body: { action: "ARCHIVE", memoUids: ["m1", "m2"] },
    });
  });

  it("requires a visibility for visibility bulk requests", () => {
    expect(buildBulkMemoRequest("VISIBILITY", ["m1"])).toEqual({
      ok: false,
      error: "请选择新的可见性",
    });
  });

  it("labels destructive bulk actions clearly", () => {
    expect(bulkMemoActionLabel("ARCHIVE")).toBe("删除");
    expect(bulkMemoActionLabel("DELETE")).toBe("彻底删除");
  });
});

describe("memo card click behavior", () => {
  const target = (matches: boolean) => ({
    closest: () => matches ? ({}) : null,
  }) as unknown as Element;

  it("opens detail when clicking non-interactive card content", () => {
    expect(shouldOpenMemoDetailFromCardClick(target(false), false)).toBe(true);
  });

  it("keeps controls inside a memo card interactive", () => {
    expect(shouldOpenMemoDetailFromCardClick(target(true), false)).toBe(false);
    expect(shouldOpenMemoDetailFromCardClick(target(false), true)).toBe(false);
  });
});

describe("webhook event catalog", () => {
  it("includes memo lifecycle, social and share events", () => {
    expect(MEMO_WEBHOOK_EVENTS).toEqual(expect.arrayContaining([
      "memo.created",
      "memo.updated",
      "memo.archived",
      "memo.restored",
      "memo.deleted",
      "memo.bulk.updated",
      "memo.comment.created",
      "reaction.upserted",
      "reaction.deleted",
      "share.created",
      "share.deleted",
    ]));
  });
});

describe("webhook delivery view helpers", () => {
  it("labels successful deliveries with HTTP status", () => {
    expect(webhookDeliveryStatusMeta({ status: "SUCCESS", statusCode: 204 })).toEqual({
      label: "成功 204",
      className: "success",
      canRetry: false,
    });
  });

  it("labels failed deliveries as retryable", () => {
    expect(webhookDeliveryStatusMeta({ status: "FAILED", statusCode: null })).toEqual({
      label: "失败",
      className: "failed",
      canRetry: true,
    });
  });

  it("formats delivery time from unix seconds", () => {
    expect(webhookDeliveryTimeLabel(1779345600)).toContain("2026");
  });
});

describe("auth flow helpers", () => {
  it("builds a login redirect path with the original route", () => {
    expect(buildAuthRedirectPath("/settings?tab=integrations")).toBe(
      "/auth?redirect=%2Fsettings%3Ftab%3Dintegrations"
    );
  });

  it("does not redirect back to the auth page", () => {
    expect(buildAuthRedirectPath("/auth")).toBe("/auth");
  });
});

describe("inbox view helpers", () => {
  it("formats comment notifications with sender and target", () => {
    expect(formatInboxItem({
      id: 1,
      createdTs: 100,
      status: "UNREAD",
      sender: { id: 2, username: "alice", nickname: "Alice" },
      message: { type: "memo.comment.created", memoUid: "m1", commentUid: "c1" },
    })).toEqual({
      title: "Alice 评论了你的备忘录",
      detail: "打开备忘录查看回复",
      memoPath: "/memos/m1",
    });
  });
});

describe("integration helpers", () => {
  it("builds public share URLs with hash routing", () => {
    expect(buildShareUrl("https://example.test", "s123")).toBe("https://example.test/#/shares/s123");
  });

  it("normalizes webhook form data", () => {
    expect(normalizeWebhookForm("  CI  ", " https://example.test/hook ")).toEqual({
      ok: true,
      name: "CI",
      url: "https://example.test/hook",
    });
  });

  it("rejects invalid webhook URLs", () => {
    expect(normalizeWebhookForm("CI", "not-url")).toEqual({
      ok: false,
      error: "请输入有效的 Webhook URL",
    });
  });
});
