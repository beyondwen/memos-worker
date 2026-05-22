use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiSettings {
    pub(crate) base_url: String,
    pub(crate) model: String,
    pub(crate) api_key: String,
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
