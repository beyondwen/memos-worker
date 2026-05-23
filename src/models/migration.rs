use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MigrationRequest {
    pub(crate) base_url: Option<String>,
    pub(crate) access_token: Option<String>,
    pub(crate) include_archived: Option<bool>,
}

#[derive(Debug, Clone)]
pub(crate) struct MigrationOptions {
    pub(crate) base_url: String,
    pub(crate) access_token: String,
    pub(crate) include_archived: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OriginalMemo {
    pub(crate) name: Option<String>,
    pub(crate) state: Option<String>,
    pub(crate) creator: Option<String>,
    pub(crate) create_time: Option<Value>,
    pub(crate) update_time: Option<Value>,
    pub(crate) content: Option<String>,
    pub(crate) visibility: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) pinned: Option<bool>,
    pub(crate) attachments: Option<Vec<Value>>,
    pub(crate) relations: Option<Vec<Value>>,
    pub(crate) property: Option<Value>,
    pub(crate) parent: Option<String>,
    pub(crate) location: Option<Value>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MigrationSummary {
    pub(crate) memo_count: usize,
    pub(crate) attachment_count: usize,
    pub(crate) relation_count: usize,
    pub(crate) archived_count: usize,
    pub(crate) truncated: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MigrationProgress {
    pub(crate) phase: String,
    pub(crate) processed: usize,
    pub(crate) imported: usize,
    pub(crate) skipped: usize,
    pub(crate) memo_count: usize,
    pub(crate) attachment_count: usize,
    pub(crate) relation_count: usize,
    pub(crate) archived_count: usize,
    pub(crate) truncated: bool,
    pub(crate) state: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OriginalBackupResult {
    pub(crate) memo_count: usize,
    pub(crate) pushed: usize,
    pub(crate) skipped: usize,
    pub(crate) archived_count: usize,
    pub(crate) truncated: bool,
}
