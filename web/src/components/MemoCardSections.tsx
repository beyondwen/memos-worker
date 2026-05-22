import type { Memo } from "./MemoCard";
import { CustomSelect } from "./CustomSelect";

const VISIBILITY_LABEL = { PRIVATE: "私有", PROTECTED: "登录可见", PUBLIC: "公开" };
const VISIBILITY_CLASS = { PRIVATE: "vis-PRIVATE", PROTECTED: "vis-PROTECTED", PUBLIC: "vis-PUBLIC" };
const VISIBILITY_OPTIONS: Array<{ value: Memo["visibility"]; label: string }> = [
  { value: "PRIVATE", label: "私有" },
  { value: "PROTECTED", label: "登录可见" },
  { value: "PUBLIC", label: "公开" },
];

function formatMemoDate(ts: number) {
  const d = new Date(ts * 1000);
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function MemoCardHeader({ memo }: { memo: Memo }) {
  return (
    <div class="memo-header">
      <span class="memo-creator">{memo.creator.nickname || memo.creator.username}</span>
      <span class="memo-dot">·</span>
      <span class="memo-time">{formatMemoDate(memo.createdTs)}</span>
      {memo.visibility !== "PRIVATE" && (
        <span class={`memo-visibility ${VISIBILITY_CLASS[memo.visibility]}`}>
          {VISIBILITY_LABEL[memo.visibility]}
        </span>
      )}
      {memo.pinned && <span style={{ fontSize: "0.75rem", color: "var(--zinc-400)" }}>📌</span>}
    </div>
  );
}

interface MemoCardEditorProps {
  content: string;
  visibility: Memo["visibility"];
  saving: boolean;
  onContentChange: (value: string) => void;
  onVisibilityChange: (value: Memo["visibility"]) => void;
  onCancel: () => void;
  onSave: () => void;
}

export function MemoCardEditor({
  content,
  visibility,
  saving,
  onContentChange,
  onVisibilityChange,
  onCancel,
  onSave,
}: MemoCardEditorProps) {
  return (
    <div>
      <textarea
        class="editor-textarea"
        value={content}
        onInput={(e) => onContentChange((e.target as HTMLTextAreaElement).value)}
        style={{ minHeight: "80px", border: "1px solid var(--zinc-200)", borderRadius: "6px", padding: "10px 12px" }}
      />
      <div class="editor-actions">
        <CustomSelect
          value={visibility}
          options={VISIBILITY_OPTIONS}
          onChange={onVisibilityChange}
          ariaLabel="可见性"
          compact
        />
        <div class="spacer" />
        <button class="btn btn-ghost btn-sm" onClick={onCancel}>取消</button>
        <button class="btn btn-primary btn-sm" onClick={onSave} disabled={saving}>
          {saving ? "保存中..." : "保存"}
        </button>
      </div>
    </div>
  );
}
