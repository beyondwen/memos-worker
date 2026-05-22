use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiSettings {
    pub(crate) base_url: String,
    pub(crate) model: String,
    pub(crate) api_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbWebhook {
    pub(crate) id: i64,
    pub(crate) created_ts: i64,
    pub(crate) updated_ts: i64,
    pub(crate) row_status: String,
    pub(crate) name: String,
    pub(crate) url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbWebhookDelivery {
    pub(crate) id: i64,
    pub(crate) webhook_id: i64,
    pub(crate) created_ts: i64,
    pub(crate) event: String,
    pub(crate) status: String,
    pub(crate) status_code: Option<i64>,
    pub(crate) duration_ms: i64,
    pub(crate) error: String,
    pub(crate) request_body: String,
    pub(crate) response_body: String,
    pub(crate) webhook_name: Option<String>,
    pub(crate) webhook_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbAuditLog {
    pub(crate) id: i64,
    pub(crate) created_ts: i64,
    pub(crate) actor_id: Option<i64>,
    pub(crate) actor_username: Option<String>,
    pub(crate) action: String,
    pub(crate) target: String,
    pub(crate) detail: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbInboxRow {
    pub(crate) id: i64,
    pub(crate) created_ts: i64,
    pub(crate) sender_id: Option<i64>,
    pub(crate) status: String,
    pub(crate) message: String,
    pub(crate) sender_username: Option<String>,
    pub(crate) sender_nickname: Option<String>,
}
