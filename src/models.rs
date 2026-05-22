use super::*;

#[derive(Debug)]
pub(crate) struct AppError {
    pub(crate) status: u16,
    pub(crate) message: String,
}

impl AppError {
    pub(crate) fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<worker::Error> for AppError {
    fn from(error: worker::Error) -> Self {
        Self::new(500, error.to_string())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicUser {
    pub(crate) id: i64,
    pub(crate) username: String,
    pub(crate) role: String,
    pub(crate) nickname: String,
    pub(crate) email: String,
    pub(crate) avatar_url: String,
    pub(crate) description: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbUser {
    pub(crate) id: i64,
    pub(crate) username: String,
    pub(crate) role: String,
    pub(crate) email: String,
    pub(crate) nickname: String,
    pub(crate) password_hash: String,
    pub(crate) avatar_url: String,
    pub(crate) description: String,
    pub(crate) row_status: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicMemo {
    pub(crate) name: String,
    pub(crate) id: i64,
    pub(crate) uid: String,
    pub(crate) creator: MemoCreator,
    pub(crate) created_ts: i64,
    pub(crate) updated_ts: i64,
    pub(crate) row_status: String,
    pub(crate) content: String,
    pub(crate) visibility: String,
    pub(crate) pinned: bool,
    pub(crate) payload: Value,
    pub(crate) attachments: Vec<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct MemoCreator {
    pub(crate) id: i64,
    pub(crate) username: String,
    pub(crate) nickname: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbMemo {
    pub(crate) id: i64,
    pub(crate) uid: String,
    pub(crate) creator_id: i64,
    pub(crate) creator_username: String,
    pub(crate) creator_nickname: String,
    pub(crate) created_ts: i64,
    pub(crate) updated_ts: i64,
    pub(crate) row_status: String,
    pub(crate) content: String,
    pub(crate) visibility: String,
    pub(crate) pinned: i64,
    pub(crate) payload: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbMemoRelation {
    pub(crate) uid: String,
    pub(crate) content: String,
    #[serde(rename = "type")]
    pub(crate) relation_type: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbMemoEvent {
    pub(crate) id: i64,
    pub(crate) event_type: String,
    pub(crate) name: String,
    pub(crate) visibility: String,
    pub(crate) creator_id: i64,
    pub(crate) payload: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct RelationCandidate {
    pub(crate) uid: String,
    pub(crate) content: String,
    pub(crate) payload: String,
    pub(crate) updated_ts: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct RankedRelationCandidate {
    pub(crate) uid: String,
    pub(crate) content: String,
    pub(crate) score: f64,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbReaction {
    pub(crate) id: i64,
    pub(crate) created_ts: i64,
    pub(crate) reaction_type: String,
    pub(crate) creator_id: i64,
    pub(crate) creator_username: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbShare {
    pub(crate) id: i64,
    pub(crate) uid: String,
    pub(crate) creator_id: i64,
    pub(crate) created_ts: i64,
    pub(crate) expires_ts: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbAttachment {
    pub(crate) id: i64,
    pub(crate) uid: String,
    pub(crate) creator_id: i64,
    pub(crate) created_ts: i64,
    pub(crate) filename: String,
    #[serde(rename = "type")]
    pub(crate) file_type: String,
    pub(crate) size: i64,
    pub(crate) memo_id: Option<i64>,
    pub(crate) reference: String,
    pub(crate) memo_visibility: Option<String>,
    pub(crate) memo_creator_id: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbAccessToken {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) token_prefix: String,
    pub(crate) created_ts: i64,
    pub(crate) updated_ts: i64,
    pub(crate) last_used_ts: Option<i64>,
    pub(crate) expires_ts: Option<i64>,
    pub(crate) row_status: String,
}

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

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Claims {
    pub(crate) iss: String,
    pub(crate) sub: String,
    pub(crate) exp: i64,
    pub(crate) iat: i64,
    #[serde(rename = "type")]
    pub(crate) token_type: String,
    pub(crate) tid: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Viewer {
    pub(crate) id: i64,
    pub(crate) role: String,
}

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

#[derive(Debug, PartialEq)]
pub(crate) struct BackupArtifact {
    pub(crate) key: String,
    pub(crate) size: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MemoChildRoute<'a> {
    ListComments,
    CreateComment,
    ListReactions,
    UpsertReaction,
    DeleteReaction(&'a str),
    GetRelations,
    SuggestRelations,
    SetRelations,
    ListShares,
    CreateShare,
    DeleteShare(&'a str),
    Unsupported,
}
