import { describe, expect, it } from "vitest";
import { buildMemoPayload, hashPassword, sanitizeFilename, verifyPassword } from "../src/index";
import { parseFilter } from "../src/filter";

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
