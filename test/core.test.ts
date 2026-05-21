import { describe, expect, it } from "vitest";
import { buildMemoPayload, hashPassword, sanitizeFilename, verifyPassword } from "../src/index";
import { parseFilter } from "../src/filter";
import { buildWebhookTestBody, deliveryStatusFromResponse, formatWebhookError } from "../src/services/webhook";
import { backupRetentionCutoff, buildBackupObjectKey, previewBackupPayload } from "../src/services/backup";
import { normalizeTagName, replaceTagInContent } from "../src/services/tags";
import { auditActionLabel } from "../src/services/audit";
import { mapOriginalMemoToImport, normalizeMemosBaseUrl, summarizeOriginalMemos } from "../src/services/migration";
import { parseAiRelationSuggestions, rankRelationCandidates } from "../src/services/aiRelations";

describe("password hashing", () => {
  it("verifies a valid PBKDF2 password and rejects a wrong password", async () => {
    const stored = await hashPassword("correct horse battery staple", 1000);

    await expect(verifyPassword("correct horse battery staple", stored)).resolves.toBe(true);
    await expect(verifyPassword("wrong password", stored)).resolves.toBe(false);
  });
});

describe("memo payload", () => {
  it("extracts tags and common content properties", () => {
    const payload = buildMemoPayload("hello #work\n- [ ] ship it\nhttps://example.com\n`code`");

    expect(payload).toEqual({
      tags: ["work"],
      property: {
        hasTaskList: true,
        hasLink: true,
        hasCode: true,
        hasIncompleteTasks: true
      }
    });
  });
});

describe("filename sanitizing", () => {
  it("removes path separators and unsafe characters", () => {
    expect(sanitizeFilename("../bad:name?.png")).toBe(".._bad_name_.png");
    expect(sanitizeFilename("\u0000")).toBe("attachment");
  });
});

describe("CEL filter engine", () => {
  it("parses simple equality filter", () => {
    const result = parseFilter('visibility == "PUBLIC"');
    expect(result).not.toBeNull();
    expect(result!.sql).toBe("memo.visibility = ?");
    expect(result!.params).toEqual(["PUBLIC"]);
    expect(result!.needsTagJoin).toBe(false);
  });

  it("parses boolean field shorthand", () => {
    const result = parseFilter("has_task_list");
    expect(result).not.toBeNull();
    expect(result!.sql).toBe("json_extract(memo.payload, '$.property.hasTaskList') = 1");
  });

  it("parses AND/OR compound expressions", () => {
    const result = parseFilter('visibility == "PUBLIC" && has_link');
    expect(result).not.toBeNull();
    expect(result!.sql).toContain("AND");
    expect(result!.params).toEqual(["PUBLIC"]);
  });

  it("parses tag filter with IN", () => {
    const result = parseFilter('tag in ("work", "personal")');
    expect(result).not.toBeNull();
    expect(result!.needsTagJoin).toBe(true);
    expect(result!.params).toEqual(["work", "personal"]);
  });

  it("parses comparison operators", () => {
    const result = parseFilter("created_ts > 1700000000");
    expect(result).not.toBeNull();
    expect(result!.sql).toBe("memo.created_ts > ?");
    expect(result!.params).toEqual([1700000000]);
  });

  it("parses content contains", () => {
    const result = parseFilter('content.contains("hello")');
    expect(result).not.toBeNull();
    expect(result!.sql).toContain("LIKE");
    expect(result!.params).toEqual(["hello"]);
  });

  it("parses NOT expression", () => {
    const result = parseFilter("!has_code");
    expect(result).not.toBeNull();
    expect(result!.sql).toContain("NOT");
  });

  it("parses creator field", () => {
    const result = parseFilter('creator == "admin"');
    expect(result).not.toBeNull();
    expect(result!.sql).toBe('"user".username = ?');
    expect(result!.params).toEqual(["admin"]);
  });

  it("returns null for empty expression", () => {
    expect(parseFilter("")).toBeNull();
    expect(parseFilter("   ")).toBeNull();
  });

  it("returns null for invalid expression", () => {
    expect(parseFilter("!!!invalid{{{")).toBeNull();
  });

  it("parses camelCase field aliases", () => {
    const result = parseFilter('visibility == "PRIVATE" && creatorId == 1');
    expect(result).not.toBeNull();
    expect(result!.sql).toContain("memo.visibility = ?");
    expect(result!.sql).toContain("memo.creator_id = ?");
    expect(result!.params).toEqual(["PRIVATE", 1]);
  });

  it("parses OR expressions", () => {
    const result = parseFilter('visibility == "PUBLIC" || visibility == "PROTECTED"');
    expect(result).not.toBeNull();
    expect(result!.sql).toContain("OR");
    expect(result!.params).toEqual(["PUBLIC", "PROTECTED"]);
  });

  it("parses startsWith method", () => {
    const result = parseFilter('content.startsWith("hello")');
    expect(result).not.toBeNull();
    expect(result!.sql).toContain("LIKE");
    expect(result!.params).toEqual(["hello"]);
  });

  it("parses endsWith method", () => {
    const result = parseFilter('content.endsWith("world")');
    expect(result).not.toBeNull();
    expect(result!.sql).toContain("LIKE");
    expect(result!.params).toEqual(["world"]);
  });

  it("parses parenthesized expressions", () => {
    const result = parseFilter('(visibility == "PUBLIC" || visibility == "PROTECTED") && pinned == true');
    expect(result).not.toBeNull();
    expect(result!.sql).toContain("AND");
    expect(result!.params).toEqual(["PUBLIC", "PROTECTED"]);
  });

  it("parses row_status field", () => {
    const result = parseFilter('rowStatus == "ARCHIVED"');
    expect(result).not.toBeNull();
    expect(result!.sql).toBe("memo.row_status = ?");
    expect(result!.params).toEqual(["ARCHIVED"]);
  });
});

describe("webhook delivery helpers", () => {
  it("marks 2xx responses as successful deliveries", () => {
    expect(deliveryStatusFromResponse(200)).toBe("SUCCESS");
    expect(deliveryStatusFromResponse(299)).toBe("SUCCESS");
  });

  it("marks non-2xx responses and network failures as failed deliveries", () => {
    expect(deliveryStatusFromResponse(300)).toBe("FAILED");
    expect(deliveryStatusFromResponse(500)).toBe("FAILED");
    expect(deliveryStatusFromResponse(null)).toBe("FAILED");
  });

  it("formats unknown webhook errors without throwing", () => {
    expect(formatWebhookError(new Error("boom"))).toBe("boom");
    expect(formatWebhookError("bad gateway")).toBe("bad gateway");
    expect(formatWebhookError(null)).toBe("Unknown webhook error");
  });

  it("builds a test webhook event body", () => {
    expect(JSON.parse(buildWebhookTestBody(123))).toMatchObject({
      event: "webhook.test",
      timestamp: 123,
      payload: { ok: true, source: "memos-worker" }
    });
  });
});

describe("backup helpers", () => {
  it("builds stable dated backup object keys", () => {
    expect(buildBackupObjectKey(new Date("2026-05-21T07:30:15.000Z"))).toBe(
      "backups/memos-2026-05-21T07-30-15-000Z.json"
    );
  });

  it("calculates retention cutoff in seconds", () => {
    expect(backupRetentionCutoff(1_000_000, 7)).toBe(395_200);
  });

  it("previews backup payload counts", () => {
    expect(previewBackupPayload({
      memos: [{ uid: "m1" }, { uid: "m2" }],
      attachments: [{ uid: "a1" }],
      relations: [],
      users: [{ id: 1 }]
    })).toEqual({
      memoCount: 2,
      attachmentCount: 1,
      relationCount: 0,
      userCount: 1
    });
  });
});

describe("tag helpers", () => {
  it("normalizes hash-prefixed tags", () => {
    expect(normalizeTagName("  #Work/Now ")).toBe("Work/Now");
  });

  it("replaces exact hash tags in memo content", () => {
    expect(replaceTagInContent("#work and #work/log", "work", "life")).toBe("#life and #work/log");
  });
});

describe("audit helpers", () => {
  it("labels known audit actions", () => {
    expect(auditActionLabel("backup.restore")).toBe("恢复备份");
    expect(auditActionLabel("unknown.action")).toBe("unknown.action");
  });
});

describe("original Memos migration helpers", () => {
  it("normalizes original Memos base URLs", () => {
    expect(normalizeMemosBaseUrl(" https://demo.usememos.com/ ")).toBe("https://demo.usememos.com");
    expect(() => normalizeMemosBaseUrl("ftp://demo.usememos.com")).toThrow("Only http and https URLs are supported");
  });

  it("maps original Memos API records into local memo inserts", () => {
    const mapped = mapOriginalMemoToImport({
      name: "memos/101",
      state: "STATE_ARCHIVED",
      content: "hello #work",
      visibility: "VISIBILITY_PUBLIC",
      createTime: "2026-05-21T00:00:00Z",
      updateTime: "2026-05-21T01:00:00Z",
      tags: ["work"],
      pinned: true,
      attachments: [{ name: "attachments/1", filename: "a.png" }],
      relations: [{ type: "REFERENCE" }]
    }, 7);

    expect(mapped).toMatchObject({
      creatorId: 7,
      content: "hello #work",
      createdTs: 1779321600,
      updatedTs: 1779325200,
      rowStatus: "ARCHIVED",
      visibility: "PUBLIC",
      pinned: 1,
      originalName: "memos/101"
    });
    expect(mapped.payload.tags).toEqual(["work"]);
    expect(mapped.payload.source).toMatchObject({
      type: "usememos",
      originalName: "memos/101",
      attachmentCount: 1,
      relationCount: 1
    });
  });

  it("summarizes original Memos records for preview", () => {
    expect(summarizeOriginalMemos([
      { name: "memos/1", state: "NORMAL", attachments: [{ name: "a" }] },
      { name: "memos/2", state: "ARCHIVED", relations: [{ type: "REFERENCE" }] }
    ])).toEqual({
      memoCount: 2,
      attachmentCount: 1,
      relationCount: 1,
      archivedCount: 1,
      truncated: false
    });
  });
});

describe("AI relation helpers", () => {
  const current = {
    uid: "m_current",
    content: "今天研究 Memos 迁移，想把导入的数据做成知识图谱",
    payload: JSON.stringify({ tags: ["memos", "graph"] }),
    updated_ts: 2000,
  };

  it("limits and ranks local candidates before sending them to AI", () => {
    const candidates = [
      {
        uid: "m_related",
        content: "Memos 导入之后可以通过引用关系形成 graph",
        payload: JSON.stringify({ tags: ["memos"] }),
        updated_ts: 1000,
      },
      ...Array.from({ length: 60 }, (_, index) => ({
        uid: `m_${index}`,
        content: `普通日记 ${index}`,
        payload: "{}",
        updated_ts: 900 - index,
      })),
    ];

    const ranked = rankRelationCandidates(current, candidates, 30);

    expect(ranked).toHaveLength(30);
    expect(ranked[0].uid).toBe("m_related");
    expect(ranked[0].score).toBeGreaterThan(ranked[1].score);
  });

  it("parses AI suggestions and drops unknown memo IDs", () => {
    const parsed = parseAiRelationSuggestions(
      JSON.stringify({
        suggestions: [
          { memo: "memos/m_related", reason: "都在讨论 Memos 迁移", confidence: 0.82 },
          { memo: "memos/m_missing", reason: "不存在", confidence: 0.9 },
        ],
      }),
      new Map([["m_related", { uid: "m_related", content: "content" }]])
    );

    expect(parsed).toEqual([
      {
        memo: "memos/m_related",
        content: "content",
        reason: "都在讨论 Memos 迁移",
        confidence: 0.82,
        source: "ai",
      },
    ]);
  });
});
