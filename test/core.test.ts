import { describe, expect, it } from "vitest";
import { buildMemoPayload, hashPassword, sanitizeFilename, verifyPassword } from "../src/index";

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
