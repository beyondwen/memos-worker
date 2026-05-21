export interface WebhookDeliveryStatusInput {
  status: "SUCCESS" | "FAILED";
  statusCode: number | null;
}

export function webhookDeliveryStatusMeta(delivery: WebhookDeliveryStatusInput): {
  label: string;
  className: "success" | "failed";
  canRetry: boolean;
} {
  if (delivery.status === "SUCCESS") {
    return {
      label: delivery.statusCode ? `成功 ${delivery.statusCode}` : "成功",
      className: "success",
      canRetry: false,
    };
  }
  return {
    label: delivery.statusCode ? `失败 ${delivery.statusCode}` : "失败",
    className: "failed",
    canRetry: true,
  };
}

export function webhookDeliveryTimeLabel(createdTs: number): string {
  return new Date(createdTs * 1000).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
