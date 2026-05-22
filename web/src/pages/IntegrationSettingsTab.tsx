import { webhookDeliveryStatusMeta, webhookDeliveryTimeLabel } from "../webhookDeliveryView";
import type { Webhook, WebhookDelivery } from "./settingsModel";

interface IntegrationSettingsTabProps {
  webhooks: Webhook[];
  webhookName: string;
  webhookUrl: string;
  webhookSaving: boolean;
  webhookDeliveries: WebhookDelivery[];
  retryingDeliveryId: number | null;
  testingWebhookId: number | null;
  onWebhookNameChange: (value: string) => void;
  onWebhookUrlChange: (value: string) => void;
  onCreateWebhook: (event: Event) => void;
  onToggleWebhook: (webhook: Webhook) => void;
  onTestWebhook: (webhook: Webhook) => void;
  onDeleteWebhook: (webhook: Webhook) => void;
  onRetryWebhookDelivery: (delivery: WebhookDelivery) => void;
}

export function IntegrationSettingsTab({
  webhooks,
  webhookName,
  webhookUrl,
  webhookSaving,
  webhookDeliveries,
  retryingDeliveryId,
  testingWebhookId,
  onWebhookNameChange,
  onWebhookUrlChange,
  onCreateWebhook,
  onToggleWebhook,
  onTestWebhook,
  onDeleteWebhook,
  onRetryWebhookDelivery,
}: IntegrationSettingsTabProps) {
  return (
    <div class="settings-section">
      <h2>Webhook 集成</h2>
      <div class="settings-record-list">
        {webhooks.map((webhook) => (
          <div key={webhook.id} class="settings-record-row">
            <div class="settings-record-main">
              <span class="settings-record-title">{webhook.name}</span>
              <span class="settings-record-meta">{webhook.rowStatus === "NORMAL" ? "启用" : "停用"} · {webhook.url}</span>
            </div>
            <div class="settings-record-actions">
              <button class="btn btn-ghost btn-sm" onClick={() => onToggleWebhook(webhook)}>
                {webhook.rowStatus === "NORMAL" ? "停用" : "启用"}
              </button>
              <button
                class="btn btn-ghost btn-sm"
                onClick={() => onTestWebhook(webhook)}
                disabled={testingWebhookId === webhook.id}
              >
                {testingWebhookId === webhook.id ? "测试中..." : "测试"}
              </button>
              <button class="btn btn-danger-soft btn-sm" onClick={() => onDeleteWebhook(webhook)}>
                删除
              </button>
            </div>
          </div>
        ))}
        {webhooks.length === 0 && <div class="muted-line">暂无 Webhook。</div>}
      </div>
      <form onSubmit={onCreateWebhook} class="inline-form">
        <div class="form-group">
          <input
            class="form-input"
            type="text"
            placeholder="名称"
            aria-label="Webhook 名称"
            value={webhookName}
            onInput={(e) => onWebhookNameChange((e.target as HTMLInputElement).value)}
          />
        </div>
        <div class="form-group">
          <input
            class="form-input"
            type="url"
            placeholder="https://example.com/webhook"
            aria-label="Webhook 地址"
            value={webhookUrl}
            onInput={(e) => onWebhookUrlChange((e.target as HTMLInputElement).value)}
          />
        </div>
        <button class="btn btn-primary btn-sm" type="submit" disabled={webhookSaving}>
          {webhookSaving ? "创建中..." : "创建"}
        </button>
      </form>

      <div class="webhook-delivery-panel">
        <div class="settings-subtitle">最近投递</div>
        <div class="webhook-delivery-list">
          {webhookDeliveries.map((delivery) => {
            const meta = webhookDeliveryStatusMeta(delivery);
            return (
              <div key={delivery.id} class="webhook-delivery-item">
                <div class="webhook-delivery-main">
                  <span class={`delivery-status ${meta.className}`}>{meta.label}</span>
                  <span class="delivery-event">{delivery.event}</span>
                  <span class="delivery-name">{delivery.webhookName}</span>
                  <span class="delivery-time">{webhookDeliveryTimeLabel(delivery.createdTs)}</span>
                </div>
                <div class="webhook-delivery-meta">
                  <span>{delivery.durationMs}ms</span>
                  {delivery.error && <span class="delivery-error">{delivery.error}</span>}
                  {meta.canRetry && (
                    <button
                      class="btn btn-ghost btn-sm"
                      onClick={() => onRetryWebhookDelivery(delivery)}
                      disabled={retryingDeliveryId === delivery.id}
                    >
                      {retryingDeliveryId === delivery.id ? "重试中..." : "重试"}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
          {webhookDeliveries.length === 0 && <div class="muted-line">暂无投递记录。</div>}
        </div>
      </div>
    </div>
  );
}
