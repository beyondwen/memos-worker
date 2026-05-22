import type { CurrentUser } from "../App";
import type { BulkMemoAction } from "../bulkActions";
import type { MemoState, MemoVisibility } from "../memoQuery";
import { buildSearchSnippet } from "../searchResultView";
import { MemoCard, type Memo } from "./MemoCard";

interface BulkBarProps {
  selectableCount: number;
  selectedCount: number;
  allSelected: boolean;
  state: MemoState;
  bulkVisibility: MemoVisibility;
  bulkWorking: boolean;
  onToggleAll: () => void;
  onRunBulkAction: (action: BulkMemoAction) => void;
  onBulkVisibilityChange: (visibility: MemoVisibility) => void;
  onClearSelection: () => void;
}

export function BulkBar({
  selectableCount,
  selectedCount,
  allSelected,
  state,
  bulkVisibility,
  bulkWorking,
  onToggleAll,
  onRunBulkAction,
  onBulkVisibilityChange,
  onClearSelection,
}: BulkBarProps) {
  if (selectableCount === 0) return null;
  return (
    <div class={`bulk-bar${selectedCount > 0 ? " active" : ""}`}>
      <label class="bulk-select-all">
        <input
          type="checkbox"
          checked={allSelected}
          onChange={onToggleAll}
          aria-label="选择当前页可操作备忘录"
        />
        <span class="select-indicator" aria-hidden="true" />
        <span>{selectedCount > 0 ? `已选 ${selectedCount} 条` : "批量选择"}</span>
      </label>

      {selectedCount > 0 && (
        <div class="bulk-actions">
          {state === "NORMAL" ? (
            <button class="btn btn-secondary btn-sm" onClick={() => onRunBulkAction("ARCHIVE")} disabled={bulkWorking}>
              删除
            </button>
          ) : (
            <>
              <button class="btn btn-secondary btn-sm" onClick={() => onRunBulkAction("RESTORE")} disabled={bulkWorking}>
                恢复
              </button>
              <button class="btn btn-danger btn-sm" onClick={() => onRunBulkAction("DELETE")} disabled={bulkWorking}>
                彻底删除
              </button>
            </>
          )}
          <select
            class="filter-select compact"
            value={bulkVisibility}
            onChange={(e) => onBulkVisibilityChange((e.target as HTMLSelectElement).value as MemoVisibility)}
            disabled={bulkWorking}
          >
            <option value="PRIVATE">私有</option>
            <option value="PROTECTED">登录可见</option>
            <option value="PUBLIC">公开</option>
          </select>
          <button class="btn btn-secondary btn-sm" onClick={() => onRunBulkAction("VISIBILITY")} disabled={bulkWorking}>
            改可见性
          </button>
          <button class="btn btn-ghost btn-sm" onClick={onClearSelection} disabled={bulkWorking}>
            清空
          </button>
        </div>
      )}
    </div>
  );
}

interface MemoListItemViewProps {
  memo: Memo;
  currentUser: CurrentUser | null;
  selected: boolean;
  selectionMode: boolean;
  search?: string;
  onUpdate: (memo: Memo) => void;
  onRemove: (uid: string) => void;
  onSelect: (uid: string, checked: boolean) => void;
  onLongPressStart: (memo: Memo) => void;
  onLongPressCancel: () => void;
}

export function MemoListItemView({
  memo,
  currentUser,
  selected,
  selectionMode,
  search,
  onUpdate,
  onRemove,
  onSelect,
  onLongPressStart,
  onLongPressCancel,
}: MemoListItemViewProps) {
  const isSelectable = currentUser && memo.creator.id === currentUser.id;
  return (
    <div
      class={`memo-list-item${isSelectable ? " selectable" : ""}${selected ? " selected" : ""}`}
      onTouchStart={() => onLongPressStart(memo)}
      onTouchEnd={onLongPressCancel}
      onTouchMove={onLongPressCancel}
      onTouchCancel={onLongPressCancel}
    >
      {isSelectable && (
        <label class="memo-select">
          <input
            type="checkbox"
            checked={selected}
            onChange={(e) => onSelect(memo.uid, (e.target as HTMLInputElement).checked)}
            aria-label="选择备忘录"
          />
          <span class="select-indicator" aria-hidden="true" />
        </label>
      )}
      <MemoCard
        memo={memo}
        currentUser={currentUser}
        onUpdate={onUpdate}
        onRemove={onRemove}
        selectionMode={selectionMode}
        selected={selected}
        onSelect={onSelect}
        highlight={search}
      />
      {search && (
        <div class="search-snippet">
          {buildSearchSnippet(memo.content, search, 44)}
        </div>
      )}
    </div>
  );
}

export function EmptyMemoList({ text }: { text: string }) {
  return (
    <div class="empty-state">
      <div class="empty-state-icon">📝</div>
      {text}
    </div>
  );
}

export function MemoListLoading() {
  return (
    <div class="loading-screen" style={{ minHeight: "120px" }}>
      <span class="loading-spinner" />
    </div>
  );
}
