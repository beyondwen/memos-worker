import { describe, expect, it } from "vitest";
import { buildMemoListPath } from "../web/src/memoQuery";
import {
  clearEditorDraft,
  loadEditorDraft,
  saveEditorDraft,
  type StorageLike,
} from "../web/src/editorDraft";

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
