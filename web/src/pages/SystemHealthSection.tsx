import type { RelationRebuildProgress, SystemHealth } from "./settingsModel";

interface SystemHealthSectionProps {
  health: SystemHealth | null;
  healthLoading: boolean;
  rebuildingIndex: boolean;
  rebuildingRelations: boolean;
  relationRebuildProgress: RelationRebuildProgress | null;
  onRefreshHealth: () => void;
  onRebuildMemoIndex: () => void;
  onRebuildRelations: () => void;
}

export function SystemHealthSection({
  health,
  healthLoading,
  rebuildingIndex,
  rebuildingRelations,
  relationRebuildProgress,
  onRefreshHealth,
  onRebuildMemoIndex,
  onRebuildRelations,
}: SystemHealthSectionProps) {
  const index = health?.memoIndex;
  const backup = health?.backup;
  return (
    <div class="settings-section">
      <h2>系统健康</h2>
      <div class="settings-actions">
        <span class="muted-line">
          {health ? `${health.status === "healthy" ? "正常" : "需处理"} · ${new Date(health.checkedTs * 1000).toLocaleString("zh-CN")}` : "尚未检查"}
        </span>
        <button class="btn btn-secondary btn-sm" onClick={onRefreshHealth} disabled={healthLoading}>
          {healthLoading ? "检查中..." : "重新检查"}
        </button>
        <button class="btn btn-ghost btn-sm" onClick={onRebuildMemoIndex} disabled={rebuildingIndex}>
          {rebuildingIndex ? "重建中..." : "重建索引"}
        </button>
        <button class="btn btn-ghost btn-sm" onClick={onRebuildRelations} disabled={rebuildingRelations}>
          {rebuildingRelations ? "关联中..." : "补充全库 AI 关联"}
        </button>
      </div>
      {relationRebuildProgress && (
        <div class="migration-progress relation-rebuild-progress">
          <div class="migration-progress-track" aria-hidden="true">
            <div
              class="migration-progress-fill determinate"
              style={{ width: `${relationRebuildPercent(relationRebuildProgress)}%` }}
            />
          </div>
          <div class="migration-progress-text">
            <strong>{relationRebuildProgress.done ? "全库关联完成" : "正在重建知识关联"}</strong>
            <span>
              已处理 {relationRebuildProgress.processed} / {relationRebuildProgress.total} 篇，写入 {relationRebuildProgress.created} 条关联
            </span>
          </div>
          {relationRebuildProgress.warnings.length > 0 && (
            <div class="muted-line">部分批次 AI 不可用，已使用本地候选补齐。</div>
          )}
        </div>
      )}
      {health && (
        <div class="settings-record-list">
          <div class="settings-record-row">
            <div class="settings-record-main">
              <span class="settings-record-title">Memo 索引</span>
              <span class="settings-record-meta">
                memo {index?.memoCount ?? 0} · search {index?.searchCount ?? 0} · tag {index?.tagCount ?? 0}
              </span>
            </div>
            <span class={`delivery-event${index?.healthy ? "" : " error"}`}>
              {index?.healthy ? "一致" : `缺失 ${index?.missingSearchCount ?? 0} / 孤儿 ${index?.orphanSearchCount ?? 0}`}
            </span>
          </div>
          <div class="settings-record-row">
            <div class="settings-record-main">
              <span class="settings-record-title">备份</span>
              <span class="settings-record-meta">
                {backup?.count ?? 0} 个备份 · {backup?.latest?.key?.split("/").pop() || "暂无最新备份"}
              </span>
            </div>
            <span class="delivery-event">
              {backup?.encryption.configured ? `加密 ${backup.encryption.currentKeyId || ""}` : "未加密"}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

function relationRebuildPercent(progress: RelationRebuildProgress) {
  if (!progress.total) return progress.done ? 100 : 0;
  return Math.min(100, Math.round((progress.processed / progress.total) * 100));
}
