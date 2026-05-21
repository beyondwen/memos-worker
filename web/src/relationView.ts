export interface RelationInput {
  memo: string;
  type: "REFERENCE";
}

export interface MemoRelation {
  memo: string;
  type: string;
  direction: "outgoing" | "incoming";
  content?: string;
}

export interface RelationSuggestion {
  memo: string;
  content: string;
  reason: string;
  confidence: number;
  source: "ai" | "local";
}

export function memoUidFromRef(value: string): string {
  const trimmed = value.trim();
  const pathMatch = trimmed.match(/\/memos\/([^/?#\s,]+)/);
  if (pathMatch) return pathMatch[1];
  const hashMatch = trimmed.match(/#\/memos\/([^/?#\s,]+)/);
  if (hashMatch) return hashMatch[1];
  return trimmed.replace(/^memos\//, "");
}

export function parseRelationInput(input: string): RelationInput[] {
  const seen = new Set<string>();
  const refs: RelationInput[] = [];
  for (const raw of input.split(/[\s,]+/)) {
    const uid = memoUidFromRef(raw);
    if (!uid || seen.has(uid)) continue;
    seen.add(uid);
    refs.push({ memo: `memos/${uid}`, type: "REFERENCE" });
  }
  return refs;
}

export function relationInputFromRelations(relations: MemoRelation[]): string {
  return relations
    .filter((relation) => relation.direction === "outgoing")
    .map((relation) => relation.memo.replace(/^memos\//, ""))
    .join("\n");
}

export function mergeRelationInputWithSuggestions(input: string, suggestions: RelationSuggestion[]): string {
  const existing = parseRelationInput(input).map((relation) => relation.memo.replace(/^memos\//, ""));
  const seen = new Set(existing);
  for (const suggestion of suggestions) {
    const uid = memoUidFromRef(suggestion.memo);
    if (!uid || seen.has(uid)) continue;
    seen.add(uid);
    existing.push(uid);
  }
  return existing.join("\n");
}
