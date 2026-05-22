import type { MemoVisibility } from "./memoQuery";

const DRAFT_KEY = "memos_editor_draft_v1";
const VISIBILITIES = new Set<MemoVisibility>(["PRIVATE", "PROTECTED", "PUBLIC"]);

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export interface EditorDraft {
  content: string;
  visibility: MemoVisibility;
  attachmentUids: string[];
  createdAt: string;
  savedAt: number;
}

interface DraftInput {
  content: string;
  visibility: MemoVisibility;
  attachmentUids: string[];
  createdAt?: string;
}

function getBrowserStorage(): StorageLike | null {
  return typeof window === "undefined" ? null : window.localStorage;
}

export function loadEditorDraft(storage: StorageLike | null = getBrowserStorage()): EditorDraft | null {
  if (!storage) return null;
  try {
    const raw = storage.getItem(DRAFT_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<EditorDraft>;
    if (typeof parsed.content !== "string") return null;
    if (!VISIBILITIES.has(parsed.visibility as MemoVisibility)) return null;
    return {
      content: parsed.content,
      visibility: parsed.visibility as MemoVisibility,
      attachmentUids: Array.isArray(parsed.attachmentUids)
        ? parsed.attachmentUids.filter((uid): uid is string => typeof uid === "string")
        : [],
      createdAt: typeof parsed.createdAt === "string" ? parsed.createdAt : "",
      savedAt: typeof parsed.savedAt === "number" ? parsed.savedAt : 0,
    };
  } catch (err) {
    console.warn("[editor-draft] draft parse failed:", err);
    return null;
  }
}

export function saveEditorDraft(
  storage: StorageLike | null,
  draft: DraftInput,
  savedAt = Date.now()
): void {
  if (!storage) return;
  if (!draft.content.trim() && draft.attachmentUids.length === 0) {
    clearEditorDraft(storage);
    return;
  }
  storage.setItem(DRAFT_KEY, JSON.stringify({ ...draft, savedAt }));
}

export function clearEditorDraft(storage: StorageLike | null = getBrowserStorage()): void {
  storage?.removeItem(DRAFT_KEY);
}
