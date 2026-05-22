import type { SystemHealth } from "./settingsModel";

interface SystemHealthSectionProps {
  health: SystemHealth | null;
  healthLoading: boolean;
  rebuildingIndex: boolean;
  onRefreshHealth: () => void;
  onRebuildMemoIndex: () => void;
}

export function SystemHealthSection({
  health,
  healthLoading,
  rebuildingIndex,
  onRefreshHealth,
  onRebuildMemoIndex,
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
      </div>
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
