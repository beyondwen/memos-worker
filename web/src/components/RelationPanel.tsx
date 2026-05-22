import { useCallback, useEffect, useState } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import {
  mergeRelationInputWithSuggestions,
  parseRelationInput,
  relationInputFromRelations,
  type MemoRelation,
  type RelationSuggestion,
} from "../relationView";
import { useFeedback } from "./Feedback";

interface RelationSuggestionResponse {
  suggestions: RelationSuggestion[];
  source?: "ai" | "local";
  warning?: string;
}

interface RelationPanelProps {
  memoUid: string;
  canEdit: boolean;
}

export function RelationPanel({ memoUid, canEdit }: RelationPanelProps) {
  const { notify } = useFeedback();
  const [relations, setRelations] = useState<MemoRelation[]>([]);
  const [input, setInput] = useState("");
  const [saving, setSaving] = useState(false);
  const [suggesting, setSuggesting] = useState(false);
  const [suggestions, setSuggestions] = useState<RelationSuggestion[]>([]);

  const fetchRelations = useCallback(async () => {
    const data = await api<{ relations: MemoRelation[] }>(`/api/v1/memos/${memoUid}/relations`);
    setRelations(data.relations);
    setInput(relationInputFromRelations(data.relations));
  }, [memoUid]);

  useEffect(() => {
    fetchRelations().catch(() => undefined);
  }, [fetchRelations]);

  const saveRelations = async () => {
    setSaving(true);
    try {
      const data = await api<{ relations: MemoRelation[] }>(`/api/v1/memos/${memoUid}/relations`, {
        method: "PATCH",
        body: JSON.stringify({ relations: parseRelationInput(input) }),
      });
      setRelations(data.relations);
      setInput(relationInputFromRelations(data.relations));
      notify("引用关系已保存", "success");
    } catch (err) {
      notify(`保存引用失败：${(err as Error).message}`, "error");
    } finally {
      setSaving(false);
    }
  };

  const suggestRelations = async () => {
    setSuggesting(true);
    try {
      const data = await api<RelationSuggestionResponse>(`/api/v1/memos/${memoUid}/relations/suggest`, {
        method: "POST",
      });
      setSuggestions(data.suggestions);
      if (data.warning) {
        notify(`AI 不可用，已使用本地推荐：${data.warning}`, "info");
      } else {
        notify(data.suggestions.length > 0 ? `识别到 ${data.suggestions.length} 条候选关联` : "暂未发现明显关联", "success");
      }
    } catch (err) {
      notify(`AI 识别失败：${(err as Error).message}`, "error");
    } finally {
      setSuggesting(false);
    }
  };

  const applySuggestions = () => {
    setInput(mergeRelationInputWithSuggestions(input, suggestions));
    notify("推荐关联已加入待保存列表", "success");
  };

  const outgoing = relations.filter((relation) => relation.direction === "outgoing");
  const incoming = relations.filter((relation) => relation.direction === "incoming");

  return (
    <div class="settings-section compact-section">
      <div class="relation-heading">
        <h2>知识关联</h2>
        {canEdit && (
          <button class="btn btn-secondary btn-sm" onClick={suggestRelations} disabled={suggesting}>
            {suggesting ? "识别中..." : "AI 识别关联"}
          </button>
        )}
      </div>
      {canEdit && (
        <div class="relation-editor">
          <textarea
            class="form-input"
            rows={3}
            placeholder="每行一个 memo uid 或 /memos/xxx 链接"
            value={input}
            onInput={(e) => setInput((e.target as HTMLTextAreaElement).value)}
          />
          <div class="relation-editor-actions">
            <button class="btn relation-save-button" onClick={saveRelations} disabled={saving || !input.trim()}>
              {saving ? "保存中..." : "保存引用"}
            </button>
          </div>
        </div>
      )}
      {suggestions.length > 0 && (
        <div class="relation-suggestions">
          <div class="relation-title">AI 推荐</div>
          <div class="relation-list">
            {suggestions.map((suggestion) => {
              const uid = suggestion.memo.replace(/^memos\//, "");
              return (
                <button key={suggestion.memo} class="relation-chip suggested" onClick={() => route(`/memos/${uid}`)}>
                  <span>{uid}</span>
                  <small>{suggestion.reason} · {Math.round(suggestion.confidence * 100)}%</small>
                </button>
              );
            })}
          </div>
          <button class="btn relation-save-button" onClick={applySuggestions}>
            应用推荐
          </button>
        </div>
      )}
      <RelationList title="引用到" relations={outgoing} />
      <RelationList title="被引用" relations={incoming} />
    </div>
  );
}

function RelationList({ title, relations }: { title: string; relations: MemoRelation[] }) {
  return (
    <div class="relation-group">
      <div class="relation-title">{title}</div>
      {relations.length === 0 ? (
        <div class="muted-line">暂无。</div>
      ) : (
        <div class="relation-list">
          {relations.map((relation) => {
            const uid = relation.memo.replace(/^memos\//, "");
            return (
              <button key={`${relation.direction}-${relation.memo}`} class="relation-chip" onClick={() => route(`/memos/${uid}`)}>
                <span>{uid}</span>
                {relation.content && <small>{relation.content.slice(0, 48)}</small>}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
