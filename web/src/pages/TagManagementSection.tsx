import type { TagItem } from "./settingsModel";

interface TagManagementSectionProps {
  tags: TagItem[];
  tagFrom: string;
  tagTo: string;
  tagSaving: boolean;
  onTagFromChange: (value: string) => void;
  onTagToChange: (value: string) => void;
  onRenameTag: (event: Event) => void;
}

export function TagManagementSection({
  tags,
  tagFrom,
  tagTo,
  tagSaving,
  onTagFromChange,
  onTagToChange,
  onRenameTag,
}: TagManagementSectionProps) {
  return (
    <div class="settings-section">
      <h2>标签管理</h2>
      <div class="tag-list settings-tag-list">
        {tags.map((tag) => (
          <button key={tag.name} class="tag-item" onClick={() => onTagFromChange(tag.name)}>
            #{tag.name} <span>{tag.count}</span>
          </button>
        ))}
        {tags.length === 0 && <div class="muted-line">暂无标签。</div>}
      </div>
      <form class="inline-form" onSubmit={onRenameTag}>
        <input class="form-input" placeholder="原标签" aria-label="原标签" value={tagFrom} onInput={(e) => onTagFromChange((e.target as HTMLInputElement).value)} />
        <input class="form-input" placeholder="新标签" aria-label="新标签" value={tagTo} onInput={(e) => onTagToChange((e.target as HTMLInputElement).value)} />
        <button class="btn btn-primary btn-sm" disabled={tagSaving || !tagFrom || !tagTo}>
          {tagSaving ? "处理中..." : "重命名/合并"}
        </button>
      </form>
    </div>
  );
}
