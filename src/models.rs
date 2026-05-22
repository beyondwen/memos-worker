#[derive(Debug)]
struct AppError {
    status: u16,
    message: String,
}

impl AppError {
    fn new(status: u16, message: impl Into<String>) -> Self {
        Self { status, message: message.into() }
    }
}

impl From<worker::Error> for AppError {
    fn from(error: worker::Error) -> Self {
        Self::new(500, error.to_string())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PublicUser {
    id: i64,
    username: String,
    role: String,
    nickname: String,
    email: String,
    avatar_url: String,
    description: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DbUser {
    id: i64,
    username: String,
    role: String,
    email: String,
    nickname: String,
    password_hash: String,
    avatar_url: String,
    description: String,
    row_status: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PublicMemo {
    name: String,
    id: i64,
    uid: String,
    creator: MemoCreator,
    created_ts: i64,
    updated_ts: i64,
    row_status: String,
    content: String,
    visibility: String,
    pinned: bool,
    payload: Value,
    attachments: Vec<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MemoCreator {
    id: i64,
    username: String,
    nickname: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DbMemo {
    id: i64,
    uid: String,
    creator_id: i64,
    creator_username: String,
    creator_nickname: String,
    created_ts: i64,
    updated_ts: i64,
    row_status: String,
    content: String,
    visibility: String,
    pinned: i64,
    payload: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DbMemoRelation {
    uid: String,
    content: String,
    #[serde(rename = "type")]
    relation_type: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DbMemoEvent {
    id: i64,
    event_type: String,
    name: String,
    visibility: String,
    creator_id: i64,
    payload: String,
}

#[derive(Debug, Deserialize, Clone)]
struct RelationCandidate {
    uid: String,
    content: String,
    payload: String,
    updated_ts: i64,
}

#[derive(Debug, Clone)]
struct RankedRelationCandidate {
    uid: String,
    content: String,
    score: f64,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct DbReaction {
    id: i64,
    created_ts: i64,
    reaction_type: String,
    creator_id: i64,
    creator_username: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DbShare {
    id: i64,
    uid: String,
    creator_id: i64,
    created_ts: i64,
    expires_ts: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
struct DbAttachment {
    id: i64,
    uid: String,
    creator_id: i64,
    created_ts: i64,
    filename: String,
    #[serde(rename = "type")]
    file_type: String,
    size: i64,
    memo_id: Option<i64>,
    reference: String,
    memo_visibility: Option<String>,
    memo_creator_id: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
struct DbAccessToken {
    id: i64,
    name: String,
    token_prefix: String,
    created_ts: i64,
    updated_ts: i64,
    last_used_ts: Option<i64>,
    expires_ts: Option<i64>,
    row_status: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiSettings {
    base_url: String,
    model: String,
    api_key: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DbWebhook {
    id: i64,
    created_ts: i64,
    updated_ts: i64,
    row_status: String,
    name: String,
    url: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DbWebhookDelivery {
    id: i64,
    webhook_id: i64,
    created_ts: i64,
    event: String,
    status: String,
    status_code: Option<i64>,
    duration_ms: i64,
    error: String,
    request_body: String,
    response_body: String,
    webhook_name: Option<String>,
    webhook_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct DbAuditLog {
    id: i64,
    created_ts: i64,
    actor_id: Option<i64>,
    actor_username: Option<String>,
    action: String,
    target: String,
    detail: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DbInboxRow {
    id: i64,
    created_ts: i64,
    sender_id: Option<i64>,
    status: String,
    message: String,
    sender_username: Option<String>,
    sender_nickname: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    iss: String,
    sub: String,
    exp: i64,
    iat: i64,
    #[serde(rename = "type")]
    token_type: String,
    tid: Option<String>,
}

#[derive(Debug, Clone)]
struct Viewer {
    id: i64,
    role: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationRequest {
    base_url: Option<String>,
    access_token: Option<String>,
    include_archived: Option<bool>,
}

#[derive(Debug, Clone)]
struct MigrationOptions {
    base_url: String,
    access_token: String,
    include_archived: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OriginalMemo {
    name: Option<String>,
    state: Option<String>,
    creator: Option<String>,
    create_time: Option<Value>,
    update_time: Option<Value>,
    content: Option<String>,
    visibility: Option<String>,
    tags: Option<Vec<String>>,
    pinned: Option<bool>,
    attachments: Option<Vec<Value>>,
    relations: Option<Vec<Value>>,
    property: Option<Value>,
    parent: Option<String>,
    location: Option<Value>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MigrationSummary {
    memo_count: usize,
    attachment_count: usize,
    relation_count: usize,
    archived_count: usize,
    truncated: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MigrationProgress {
    phase: String,
    processed: usize,
    imported: usize,
    skipped: usize,
    memo_count: usize,
    attachment_count: usize,
    relation_count: usize,
    archived_count: usize,
    truncated: bool,
    state: Option<String>,
}

#[derive(Debug, PartialEq)]
struct BackupArtifact {
    key: String,
    size: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum MemoChildRoute<'a> {
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
