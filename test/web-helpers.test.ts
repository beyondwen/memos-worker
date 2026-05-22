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
import { buildBulkMemoRequest, bulkMemoActionLabel } from "../web/src/bulkActions";
import { buildApiUrl } from "../web/src/api";
import { highlightRenderedHtml } from "../web/src/searchHighlight";
import { applyMemoTemplate, MEMO_TEMPLATES } from "../web/src/memoTemplates";
import { buildSearchSnippet, scoreSearchMatch } from "../web/src/searchResultView";
import { attachmentCleanupSummary } from "../web/src/attachmentCleanupView";
import { buildHomeDateFilterPath, parseHomeDateFilterParams, stripHomeFilterParams } from "../web/src/homeFilters";
import { shouldOpenMemoDetailFromCardClick } from "../web/src/cardClick";
import { isHeaderNavActive } from "../web/src/headerNav";
import { shouldAutoLoadNextMemoPage } from "../web/src/memoListPaging";
import { MEMO_LIST_SSE_REFRESH_DEBOUNCE_MS, scheduleDebouncedRefresh } from "../web/src/sseRefresh";
import { personalPrimaryNavItems, personalSettingsTabs } from "../web/src/personalMode";
import { buildAiSettingsPayload, buildMigrationProgressView } from "../web/src/pages/settingsPageHelpers";
import { SETTINGS_TABS } from "../web/src/pages/settingsModel";

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

describe("memo list paging", () => {
  it("auto-loads only when another page is available and no request is running", () => {
    expect(shouldAutoLoadNextMemoPage({
      hasMore: true,
      loading: false,
      nextPageToken: "cursor-1",
    })).toBe(true);

    expect(shouldAutoLoadNextMemoPage({
      hasMore: true,
      loading: true,
      nextPageToken: "cursor-1",
    })).toBe(false);

    expect(shouldAutoLoadNextMemoPage({
      hasMore: true,
      loading: false,
      nextPageToken: "",
    })).toBe(false);
  });
});

describe("SSE refresh scheduling", () => {
  it("replaces pending memo list refresh timers", () => {
    const cleared: number[] = [];
    const scheduled: number[] = [];
    const timer = scheduleDebouncedRefresh(
      7,
      (_callback, delay) => {
        scheduled.push(delay);
        return 9;
      },
      (id) => cleared.push(id),
      () => undefined,
    );

    expect(timer).toBe(9);
    expect(cleared).toEqual([7]);
    expect(scheduled).toEqual([MEMO_LIST_SSE_REFRESH_DEBOUNCE_MS]);
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
  });

  it("does not refresh personal lists for removed social/share events", () => {
    expect(shouldRefreshForSseEvent({ type: "reaction.upserted", name: "memos/a" })).toBe(false);
    expect(shouldRefreshForSseEvent({ type: "reaction.deleted", name: "memos/a" })).toBe(false);
    expect(shouldRefreshForSseEvent({ type: "share.created", name: "memos/a" })).toBe(false);
    expect(shouldRefreshForSseEvent({ type: "share.deleted", name: "memos/a" })).toBe(false);
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

  it("preserves all selected memo UIDs for bulk delete", () => {
    expect(buildBulkMemoRequest("DELETE", ["m1", "m2", "m3"])).toEqual({
      ok: true,
      body: { action: "DELETE", memoUids: ["m1", "m2", "m3"] },
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

describe("header navigation state", () => {
  it("matches the active top-level route", () => {
    expect(isHeaderNavActive("/", "/")).toBe(true);
    expect(isHeaderNavActive("/settings", "/settings")).toBe(true);
    expect(isHeaderNavActive("/settings/profile", "/settings")).toBe(true);
  });

  it("does not keep home active for other routes", () => {
    expect(isHeaderNavActive("/explore", "/")).toBe(false);
    expect(isHeaderNavActive("/settings", "/")).toBe(false);
  });
});

describe("api URL builder", () => {
  it("keeps same-origin API paths by default", () => {
    expect(buildApiUrl("/api/v1/memos")).toBe("/api/v1/memos");
  });

  it("prefixes API paths when a Pages deployment uses a separate backend origin", () => {
    expect(buildApiUrl("/api/v1/memos", "https://memos-api.example.com/")).toBe(
      "https://memos-api.example.com/api/v1/memos"
    );
  });
});

describe("personal mode feature trim", () => {
  it("keeps only personal-first primary navigation", () => {
    expect(personalPrimaryNavItems(true).map((item) => item.id)).toEqual([
      "home",
      "timeline",
      "settings",
    ]);
    expect(personalPrimaryNavItems(false).map((item) => item.id)).toEqual([
      "home",
    ]);
  });

  it("keeps only account, data and maintenance settings tabs by default", () => {
    expect(personalSettingsTabs(SETTINGS_TABS, "ADMIN").map((tab) => tab.id)).toEqual([
      "account",
      "data",
      "maintenance",
    ]);
  });
});

describe("auth flow helpers", () => {
  it("builds a login redirect path with the original route", () => {
    expect(buildAuthRedirectPath("/settings?tab=data")).toBe(
      "/auth?redirect=%2Fsettings%3Ftab%3Ddata"
    );
  });

  it("does not redirect back to the auth page", () => {
    expect(buildAuthRedirectPath("/auth")).toBe("/auth");
  });
});

describe("settings page helpers", () => {
  it("builds trimmed AI settings payloads without preserving form whitespace", () => {
    expect(buildAiSettingsPayload({
      baseUrl: " https://api.example.test/v1 ",
      model: " model-x ",
      apiKey: " sk-test ",
    })).toEqual({
      baseUrl: "https://api.example.test/v1",
      model: "model-x",
      apiKey: "sk-test",
    });
  });

  it("describes migration progress for previewing, running and done states", () => {
    expect(buildMigrationProgressView({
      previewing: true,
      importing: false,
      preview: null,
      progress: null,
    })).toMatchObject({
      visible: true,
      knownTotal: 0,
      percent: null,
      title: "正在预检源数据",
      detail: "正在读取原版 Memos 列表和元信息",
    });

    expect(buildMigrationProgressView({
      previewing: false,
      importing: true,
      preview: { memoCount: 10, attachmentCount: 0, relationCount: 0, archivedCount: 0, truncated: false },
      progress: {
        phase: "importing",
        processed: 4,
        imported: 3,
        skipped: 1,
        memoCount: 10,
        attachmentCount: 0,
        relationCount: 0,
        archivedCount: 0,
        truncated: false,
      },
    })).toMatchObject({
      visible: true,
      knownTotal: 10,
      percent: 40,
      title: "正在迁移备忘录",
      detail: "已处理 4 / 10 条，导入 3 条，跳过 1 条",
    });

    expect(buildMigrationProgressView({
      previewing: false,
      importing: false,
      preview: { memoCount: 2, attachmentCount: 0, relationCount: 0, archivedCount: 0, truncated: false },
      progress: {
        phase: "done",
        processed: 2,
        imported: 2,
        skipped: 0,
        memoCount: 2,
        attachmentCount: 0,
        relationCount: 0,
        archivedCount: 0,
        truncated: false,
      },
    })).toMatchObject({
      visible: true,
      percent: 100,
      title: "迁移完成",
    });
  });
});
