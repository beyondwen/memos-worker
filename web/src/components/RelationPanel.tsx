import { useCallback, useEffect, useState } from "preact/hooks";
import { route } from "preact-router";
import { api } from "../api";
import { parseRelationInput, relationInputFromRelations, type MemoRelation } from "../relationView";
import { useFeedback } from "./Feedback";

interface RelationPanelProps {
  memoUid: string;
  canEdit: boolean;
}

export function RelationPanel({ memoUid, canEdit }: RelationPanelProps) {
  const { notify } = useFeedback();
  const [relations, setRelations] = useState<MemoRelation[]>([]);
  const [input, setInput] = useState("");
  const [saving, setSaving] = useState(false);

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

  const outgoing = relations.filter((relation) => relation.direction === "outgoing");
  const incoming = relations.filter((relation) => relation.direction === "incoming");

  return (
    <div class="settings-section compact-section">
      <h2>引用关系</h2>
      {canEdit && (
        <div class="relation-editor">
          <textarea
            class="form-input"
            rows={3}
            placeholder="每行一个 memo uid 或 /memos/xxx 链接"
            value={input}
            onInput={(e) => setInput((e.target as HTMLTextAreaElement).value)}
          />
          <button class="btn btn-primary btn-sm" onClick={saveRelations} disabled={saving}>
            {saving ? "保存中..." : "保存引用"}
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
                {uid}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
