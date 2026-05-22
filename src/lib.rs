use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use futures_channel::mpsc;
use futures_util::StreamExt;
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use url::Url;
use wasm_bindgen::JsValue;
use worker::d1::D1Database;
use worker::*;

type HmacSha256 = Hmac<Sha256>;

const ACCESS_COOKIE: &str = "memos_access";
const REFRESH_COOKIE: &str = "memos_refresh";
const ACCESS_TTL: i64 = 15 * 60;
const REFRESH_TTL: i64 = 30 * 24 * 60 * 60;
const MIGRATION_PAGE_SIZE: usize = 100;
const MIGRATION_MAX_MEMOS: usize = 10000;
const SQL_IN_CHUNK_SIZE: usize = 50;
const MEMO_EVENT_RETENTION_DAYS: i64 = 7;

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

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    match route(&mut req, &env).await {
        Ok(response) => Ok(response),
        Err(error) => json_response(json!({ "error": error.message }), error.status),
    }
}

#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    match create_scheduled_backup(&env).await {
        Ok(artifact) => console_log!("scheduled backup created: {}", artifact.key),
        Err(error) => console_log!("scheduled backup failed: {}", error.message),
    }
    match prune_memo_events(&env, MEMO_EVENT_RETENTION_DAYS).await {
        Ok(deleted) => console_log!("memo event prune completed: {}", deleted),
        Err(error) => console_log!("memo event prune failed: {}", error.message),
    }
}

async fn route(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let url = req.url().map_err(AppError::from)?;
    let path = url.path().to_string();
    let method = req.method();

    if method == Method::Options {
        return Ok(empty_response(204));
    }

    if path == "/api/v1/instance" && method == Method::Get {
        return get_instance(env).await;
    }
    if path == "/api/v1/setup" && method == Method::Post {
        return setup_admin(req, env).await;
    }
    if path == "/api/v1/auth/signup" && method == Method::Post {
        return sign_up(req, env).await;
    }
    if path == "/api/v1/auth/signin" && method == Method::Post {
        return sign_in(req, env).await;
    }
    if path == "/api/v1/auth/refresh" && method == Method::Post {
        return refresh_session(req, env).await;
    }
    if path == "/api/v1/auth/signout" && method == Method::Post {
        return sign_out();
    }
    if path == "/api/v1/explore/rss.xml" && method == Method::Get {
        return generate_rss(env, None).await;
    }
    if path.starts_with("/api/v1/u/") && path.ends_with("/rss.xml") && method == Method::Get {
        let username = path.trim_start_matches("/api/v1/u/").trim_end_matches("/rss.xml").trim_end_matches('/');
        return generate_rss(env, Some(username)).await;
    }
    if path.starts_with("/api/v1/shares/") && method == Method::Get {
        let rest = path.trim_start_matches("/api/v1/shares/");
        if let Some((share_uid, attachment_rest)) = rest.split_once("/attachments/") {
            let attachment_uid = attachment_rest.split('/').next().unwrap_or("");
            return download_shared_attachment(env, share_uid, attachment_uid).await;
        }
        return public_share(env, rest).await;
    }

    if path.starts_with("/api/") || path.starts_with("/file/") {
        let viewer = current_viewer(req, env).await?;
        return authed_route(req, env, &url, &path, method, viewer).await;
    }

    fetch_asset(req, env).await
}

async fn authed_route(
    req: &mut Request,
    env: &Env,
    url: &Url,
    path: &str,
    method: Method,
    viewer: Viewer,
) -> std::result::Result<Response, AppError> {
    if path == "/api/v1/auth/user" && method == Method::Get {
        let user = get_user_by_id(env, viewer.id).await?.ok_or_else(|| AppError::new(401, "User unavailable"))?;
        return json_response(json!({ "user": public_user(user) }), 200).map_err(AppError::from);
    }
    if path == "/api/v1/users/me" && method == Method::Patch {
        return update_me(req, env, &viewer).await;
    }
    if path == "/api/v1/auth/change-password" && method == Method::Post {
        return change_password(req, env, &viewer).await;
    }
    if path == "/api/v1/users" && method == Method::Get {
        return list_users(env, &viewer).await;
    }
    if path == "/api/v1/memos" && method == Method::Get {
        return list_memos(env, url, &viewer).await;
    }
    if path == "/api/v1/memos" && method == Method::Post {
        return create_memo(req, env, &viewer).await;
    }
    if path == "/api/v1/memos/batch" && method == Method::Post {
        return bulk_memos(req, env, &viewer).await;
    }
    if path == "/api/v1/export/memos" && method == Method::Get {
        return export_data(env, &viewer).await;
    }
    if path == "/api/v1/import/memos" && method == Method::Post {
        return import_data(req, env, &viewer).await;
    }
    if path == "/api/v1/migration/memos/preview" && method == Method::Post {
        return migration_preview(req, env, &viewer).await;
    }
    if path == "/api/v1/migration/memos/import" && method == Method::Post {
        return migration_import(req, env, &viewer).await;
    }
    if path == "/api/v1/migration/memos/import-stream" && method == Method::Post {
        return migration_import_stream(req, env, &viewer).await;
    }
    if path == "/api/v1/tags" && method == Method::Get {
        return list_tags(env, &viewer).await;
    }
    if path == "/api/v1/tags/rename" && method == Method::Post {
        return rename_tag(req, env, &viewer).await;
    }
    if path == "/api/v1/timeline" && method == Method::Get {
        return timeline(env, &viewer).await;
    }
    if path == "/api/v1/inbox" && method == Method::Get {
        return list_inbox(env, &viewer).await;
    }
    if path == "/api/v1/inbox" && method == Method::Patch {
        return update_inbox_status(req, env, &viewer).await;
    }
    if path.starts_with("/api/v1/inbox/") && method == Method::Delete {
        let id = path.trim_start_matches("/api/v1/inbox/");
        return delete_inbox_item(env, &viewer, id).await;
    }
    if path == "/api/v1/attachments" && method == Method::Get {
        return list_attachments(env, url, &viewer).await;
    }
    if path == "/api/v1/attachments" && method == Method::Post {
        return upload_attachment(req, env, &viewer).await;
    }
    if path == "/api/v1/attachments/batch-delete" && method == Method::Post {
        return batch_delete_attachments(req, env, &viewer).await;
    }
    if path.starts_with("/api/v1/attachments/") && method == Method::Delete {
        let uid = path.trim_start_matches("/api/v1/attachments/");
        return delete_attachment(env, &viewer, uid).await;
    }
    if path.starts_with("/file/attachments/") && method == Method::Get {
        let rest = path.trim_start_matches("/file/attachments/");
        let uid = rest.split('/').next().unwrap_or("");
        return download_attachment(env, &viewer, uid).await;
    }
    if path == "/api/v1/backups" && method == Method::Get {
        return list_backups(env, &viewer).await;
    }
    if path == "/api/v1/backups" && method == Method::Post {
        return create_backup(env, &viewer).await;
    }
    if path == "/api/v1/backups/download" && method == Method::Get {
        return download_backup(env, url, &viewer).await;
    }
    if path == "/api/v1/backups/preview" && method == Method::Post {
        return preview_backup(req, env, &viewer).await;
    }
    if path == "/api/v1/backups/restore" && method == Method::Post {
        return restore_backup(req, env, &viewer).await;
    }
    if path == "/api/v1/ai/settings" && method == Method::Get {
        return get_ai_settings(env, &viewer).await;
    }
    if path == "/api/v1/ai/settings" && method == Method::Patch {
        return update_ai_settings(req, env, &viewer).await;
    }
    if path == "/api/v1/ai/settings/test" && method == Method::Post {
        return test_ai_settings(req, env, &viewer).await;
    }
    if path == "/api/v1/audit-logs" && method == Method::Get {
        return list_audit_logs(env, &viewer).await;
    }
    if path.starts_with("/api/v1/users/") && path.ends_with("/stats") && method == Method::Get {
        let identifier = path.trim_start_matches("/api/v1/users/").trim_end_matches("/stats").trim_end_matches('/');
        return user_stats(env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/") && path.ends_with("/access-tokens") && method == Method::Get {
        let identifier = path.trim_start_matches("/api/v1/users/").trim_end_matches("/access-tokens").trim_end_matches('/');
        return list_access_tokens(env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/") && path.ends_with("/access-tokens") && method == Method::Post {
        let identifier = path.trim_start_matches("/api/v1/users/").trim_end_matches("/access-tokens").trim_end_matches('/');
        return create_access_token(req, env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/") && path.contains("/access-tokens/") && method == Method::Delete {
        let rest = path.trim_start_matches("/api/v1/users/");
        if let Some((identifier, token_id)) = rest.split_once("/access-tokens/") {
            return delete_access_token(env, &viewer, identifier, token_id).await;
        }
    }
    if path == "/api/v1/webhooks" && method == Method::Get {
        return list_webhooks(env, &viewer).await;
    }
    if path == "/api/v1/webhooks" && method == Method::Post {
        return create_webhook(req, env, &viewer).await;
    }
    if path == "/api/v1/webhooks/deliveries" && method == Method::Get {
        return list_webhook_deliveries(env, url, &viewer).await;
    }
    if path.starts_with("/api/v1/webhooks/deliveries/") && path.ends_with("/retry") && method == Method::Post {
        let id = path.trim_start_matches("/api/v1/webhooks/deliveries/").trim_end_matches("/retry").trim_matches('/');
        return retry_webhook_delivery(env, &viewer, id).await;
    }
    if path.starts_with("/api/v1/webhooks/") && path.ends_with("/test") && method == Method::Post {
        let id = path.trim_start_matches("/api/v1/webhooks/").trim_end_matches("/test").trim_matches('/');
        return test_webhook(env, &viewer, id).await;
    }
    if let Some(id) = path.strip_prefix("/api/v1/webhooks/") {
        return match method {
            Method::Patch => update_webhook(req, env, &viewer, id).await,
            Method::Delete => delete_webhook(env, &viewer, id).await,
            _ => Err(AppError::new(405, "Method not allowed")),
        };
    }
    if path == "/api/v1/sse" && method == Method::Get {
        return connect_sse(req, env, url, &viewer).await;
    }

    if let Some(uid) = path.strip_prefix("/api/v1/memos/") {
        return memo_subroute(req, env, &viewer, uid, method, url).await;
    }
    if let Some(identifier) = path.strip_prefix("/api/v1/users/") {
        return user_subroute(req, env, &viewer, identifier, method).await;
    }

    Err(AppError::new(404, "Not found"))
}

async fn get_instance(env: &Env) -> std::result::Result<Response, AppError> {
    let db = db(env)?;
    let count: Option<i64> = db.prepare("SELECT COUNT(*) AS count FROM \"user\"")
        .first(Some("count"))
        .await?;
    json_response(json!({ "name": "Memos Worker", "setupRequired": count.unwrap_or(0) == 0 }), 200).map_err(AppError::from)
}

async fn setup_admin(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let db = db(env)?;
    let count: Option<i64> = db.prepare("SELECT COUNT(*) AS count FROM \"user\"")
        .first(Some("count"))
        .await?;
    if count.unwrap_or(0) > 0 {
        return Err(AppError::new(409, "Instance already initialized"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let username = normalize_username(body.get("username").and_then(Value::as_str))?;
    let password = body.get("password").and_then(Value::as_str).ok_or_else(|| AppError::new(400, "Password is required"))?;
    assert_password(password)?;
    let password_hash = hash_password(password);
    let now = unix_now();
    db.prepare("INSERT INTO \"user\" (created_ts, updated_ts, username, role, email, nickname, password_hash) VALUES (?, ?, ?, 'ADMIN', ?, ?, ?)")
        .bind(&[
            js_num(now),
            js_num(now),
            username.clone().into(),
            body.get("email").and_then(Value::as_str).unwrap_or("").into(),
            body.get("nickname").and_then(Value::as_str).unwrap_or(&username).into(),
            password_hash.into(),
        ])?
        .run()
        .await?;
    let user = get_user_by_username(env, &username).await?.ok_or_else(|| AppError::new(500, "Failed to create admin"))?;
    create_auth_response(env, req, user, 201).await
}

async fn sign_up(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let username = normalize_username(body.get("username").and_then(Value::as_str))?;
    let password = body.get("password").and_then(Value::as_str).ok_or_else(|| AppError::new(400, "Password is required"))?;
    assert_password(password)?;
    if get_user_by_username(env, &username).await?.is_some() {
        return Err(AppError::new(409, "Username already exists"));
    }
    let user_count: Option<i64> = db(env)?.prepare("SELECT COUNT(*) AS count FROM \"user\"")
        .first(Some("count"))
        .await?;
    let role = if user_count.unwrap_or(0) == 0 { "ADMIN" } else { "USER" };
    let now = unix_now();
    db(env)?.prepare("INSERT INTO \"user\" (created_ts, updated_ts, username, role, email, nickname, password_hash) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&[
            js_num(now),
            js_num(now),
            username.clone().into(),
            role.into(),
            body.get("email").and_then(Value::as_str).unwrap_or("").into(),
            body.get("nickname").and_then(Value::as_str).unwrap_or(&username).into(),
            hash_password(password).into(),
        ])?
        .run()
        .await?;
    let user = get_user_by_username(env, &username).await?.ok_or_else(|| AppError::new(500, "Failed to create user"))?;
    create_auth_response(env, req, user, 201).await
}

async fn sign_in(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let username = normalize_username(body.get("username").and_then(Value::as_str))?;
    let password = body.get("password").and_then(Value::as_str).ok_or_else(|| AppError::new(400, "Password is required"))?;
    let user = get_user_by_username(env, &username).await?
        .ok_or_else(|| AppError::new(401, "Invalid username or password"))?;
    if user.row_status != "NORMAL" || !verify_password(password, &user.password_hash) {
        return Err(AppError::new(401, "Invalid username or password"));
    }
    create_auth_response(env, req, user, 200).await
}

async fn refresh_session(req: &Request, env: &Env) -> std::result::Result<Response, AppError> {
    let cookies = parse_cookies(req.headers().get("Cookie").ok().flatten().unwrap_or_default().as_str());
    let refresh = cookies.get(REFRESH_COOKIE).ok_or_else(|| AppError::new(401, "Missing refresh token"))?;
    let claims = verify_jwt(refresh, &server_secret(env)?)?;
    if claims.token_type != "refresh" {
        return Err(AppError::new(401, "Invalid refresh token"));
    }
    let user_id = claims.sub.parse::<i64>().map_err(|_| AppError::new(401, "Invalid token subject"))?;
    let user = get_user_by_id(env, user_id).await?.ok_or_else(|| AppError::new(401, "User unavailable"))?;
    create_auth_response(env, req, user, 200).await
}

fn sign_out() -> std::result::Result<Response, AppError> {
    let mut res = json_response(json!({ "ok": true }), 200).map_err(AppError::from)?;
    append_cookie(&mut res, &clear_cookie(REFRESH_COOKIE));
    append_cookie(&mut res, &clear_cookie(ACCESS_COOKIE));
    Ok(res)
}

async fn create_auth_response(env: &Env, req: &Request, user: DbUser, status: u16) -> std::result::Result<Response, AppError> {
    let now = unix_now();
    let session_id = generate_uid("s");
    let refresh_token = sign_jwt(&Claims {
        iss: "memos-worker".to_string(),
        sub: user.id.to_string(),
        exp: now + REFRESH_TTL,
        iat: now,
        token_type: "refresh".to_string(),
        tid: Some(session_id.clone()),
    }, &server_secret(env)?)?;
    let access_token = sign_jwt(&Claims {
        iss: "memos-worker".to_string(),
        sub: user.id.to_string(),
        exp: now + ACCESS_TTL,
        iat: now,
        token_type: "access".to_string(),
        tid: Some(session_id.clone()),
    }, &server_secret(env)?)?;
    let token_hash = sha256_hex(&refresh_token);
    let ua = req.headers().get("User-Agent").ok().flatten().unwrap_or_default();
    db(env)?.prepare("INSERT OR REPLACE INTO user_session (id, user_id, refresh_token_hash, created_ts, updated_ts, expires_ts, user_agent, ip_address) VALUES (?, ?, ?, ?, ?, ?, ?, '')")
        .bind(&[session_id.into(), js_num(user.id), token_hash.into(), js_num(now), js_num(now), js_num(now + REFRESH_TTL), ua.into()])?
        .run()
        .await?;
    let mut res = json_response(json!({ "accessToken": access_token, "user": public_user(user) }), status).map_err(AppError::from)?;
    append_cookie(&mut res, &cookie(REFRESH_COOKIE, &refresh_token, REFRESH_TTL, true));
    Ok(res)
}

async fn current_viewer(req: &Request, env: &Env) -> std::result::Result<Viewer, AppError> {
    let mut token = None;
    if let Ok(Some(auth)) = req.headers().get("Authorization") {
        if let Some(rest) = auth.strip_prefix("Bearer ") {
            token = Some(rest.trim().to_string());
        }
    }
    if token.is_none() {
        if let Ok(url) = req.url() {
            token = query_param(&url, "access_token").or_else(|| query_param(&url, "token"));
        }
    }
    if token.is_none() {
        let cookies = parse_cookies(req.headers().get("Cookie").ok().flatten().unwrap_or_default().as_str());
        token = cookies.get(ACCESS_COOKIE).cloned();
    }
    let token = token.ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    if token.starts_with("memos_pat_") {
        return viewer_from_access_token(env, &token).await;
    }
    let claims = verify_jwt(&token, &server_secret(env)?)?;
    if claims.token_type != "access" {
        return Err(AppError::new(401, "Unauthorized"));
    }
    let id = claims.sub.parse::<i64>().map_err(|_| AppError::new(401, "Unauthorized"))?;
    let user = get_user_by_id(env, id).await?.ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    Ok(Viewer { id: user.id, role: user.role })
}

async fn viewer_from_access_token(env: &Env, token: &str) -> std::result::Result<Viewer, AppError> {
    let prefix: String = token.chars().take(20).collect();
    let token_hash = sha256_hex(token);
    let row: Option<Value> = db(env)?.prepare("SELECT user_id FROM user_access_token WHERE token_prefix = ? AND token_hash = ? AND row_status = 'NORMAL' AND (expires_ts IS NULL OR expires_ts > ?)")
        .bind(&[prefix.into(), token_hash.clone().into(), js_num(unix_now())])?
        .first(None)
        .await?;
    let user_id = row
        .and_then(|value| value.get("user_id").and_then(Value::as_i64).or_else(|| value.get("USER_ID").and_then(Value::as_i64)).map(|id| id))
        .ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    db(env)?.prepare("UPDATE user_access_token SET last_used_ts = ? WHERE token_hash = ?")
        .bind(&[js_num(unix_now()), token_hash.into()])?
        .run()
        .await?;
    let user = get_user_by_id(env, user_id).await?.ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    Ok(Viewer { id: user.id, role: user.role })
}

async fn list_memos(env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let db = db(env)?;
    let limit = url.query_pairs()
        .find(|(key, _)| key == "page_size" || key == "pageSize")
        .and_then(|(_, value)| value.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 200);
    let offset = query_param(url, "page_token")
        .or_else(|| query_param(url, "pageToken"))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let state = query_param(url, "state")
        .or_else(|| extract_filter_value(url, "rowStatus"))
        .or_else(|| extract_filter_value(url, "row_status"))
        .unwrap_or_else(|| "NORMAL".to_string());
    let state = normalize_state(&state)?;
    let mut where_sql = vec!["memo.row_status = ?".to_string()];
    let mut values = vec![state.into()];
    if viewer.role != "ADMIN" {
        where_sql.push("(memo.visibility != 'PRIVATE' OR memo.creator_id = ?)".to_string());
        values.push(js_num(viewer.id));
    }
    if let Some(visibility) = query_param(url, "visibility").filter(|value| !value.trim().is_empty()) {
        where_sql.push("memo.visibility = ?".to_string());
        values.push(normalize_visibility(&visibility)?.into());
    }
    if let Some(tag) = query_param(url, "tag").filter(|value| !value.trim().is_empty()) {
        where_sql.push("EXISTS (SELECT 1 FROM json_each(memo.payload, '$.tags') WHERE value = ?)".to_string());
        values.push(tag.into());
    }
    if let Some(search) = extract_content_contains_filter(url).filter(|value| !value.trim().is_empty()) {
        where_sql.push("memo.content LIKE ? ESCAPE '\\'".to_string());
        values.push(format!("%{}%", escape_like(&search)).into());
    }
    values.push(js_num(limit + 1));
    values.push(js_num(offset));
    let rows = db.prepare(format!(
        "SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE {} ORDER BY memo.pinned DESC, memo.created_ts DESC, memo.id DESC LIMIT ? OFFSET ?",
        where_sql.join(" AND ")
    ))
        .bind(&values)?
        .all()
        .await?;
    let memos: Vec<DbMemo> = rows.results()?;
    let has_more = memos.len() as i64 > limit;
    let mut public = Vec::new();
    for memo in memos.into_iter().take(limit as usize) {
        public.push(memo_with_attachments(env, memo).await?);
    }
    let next_page_token = if has_more { (offset + limit).to_string() } else { String::new() };
    json_response(json!({ "memos": public, "nextPageToken": next_page_token }), 200).map_err(AppError::from)
}

async fn create_memo(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let content = body.get("content").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Err(AppError::new(400, "Content is required"));
    }
    let visibility = normalize_visibility(body.get("visibility").and_then(Value::as_str).unwrap_or("PRIVATE"))?;
    let uid = generate_uid("m");
    let now = unix_now();
    let payload = build_memo_payload(&content);
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, payload) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&[uid.clone().into(), js_num(viewer.id), js_num(now), js_num(now), content.into(), visibility.into(), payload.to_string().into()])?
        .run()
        .await?;
    let memo = get_memo_by_uid(env, &uid).await?.ok_or_else(|| AppError::new(500, "Failed to create memo"))?;
    emit_memo_event(env, "memo.created", &memo).await;
    let memo = memo_with_attachments(env, memo).await?;
    json_response(json!({ "memo": memo }), 201).map_err(AppError::from)
}

async fn memo_subroute(req: &mut Request, env: &Env, viewer: &Viewer, raw: &str, method: Method, url: &Url) -> std::result::Result<Response, AppError> {
    let parts: Vec<&str> = raw.split('/').collect();
    let uid = parts[0];
    if parts.len() > 1 {
        match memo_child_route(&parts, &method) {
            MemoChildRoute::ListComments => return list_comments(env, viewer, uid).await,
            MemoChildRoute::CreateComment => return create_comment(req, env, viewer, uid).await,
            MemoChildRoute::ListReactions => return list_reactions(env, viewer, uid).await,
            MemoChildRoute::UpsertReaction => return upsert_reaction(req, env, viewer, uid).await,
            MemoChildRoute::DeleteReaction(reaction_id) => return delete_reaction(env, viewer, uid, reaction_id).await,
            MemoChildRoute::GetRelations => return get_relations(env, viewer, uid).await,
            MemoChildRoute::SuggestRelations => return suggest_memo_relations(env, viewer, uid).await,
            MemoChildRoute::SetRelations => return set_relations(req, env, viewer, uid).await,
            MemoChildRoute::ListShares => return list_shares(env, viewer, uid).await,
            MemoChildRoute::CreateShare => return create_share(req, env, viewer, uid).await,
            MemoChildRoute::DeleteShare(share_id) => return delete_share(env, viewer, uid, share_id).await,
            MemoChildRoute::Unsupported => return Err(AppError::new(404, "Memo subroute not found")),
        }
    }
    match method {
        Method::Get => {
            let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
            if !can_read(&memo, viewer) {
                return Err(AppError::new(403, "Forbidden"));
            }
            let memo = memo_with_attachments(env, memo).await?;
            json_response(json!({ "memo": memo }), 200).map_err(AppError::from)
        }
        Method::Patch => update_memo(req, env, viewer, uid).await,
        Method::Delete => {
            let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
            if !can_write(&memo, viewer) {
                return Err(AppError::new(403, "Forbidden"));
            }
            if url.query_pairs().any(|(k, v)| k == "purge" && v == "true") {
                purge_ids(env, &[memo.id]).await?;
                emit_memo_event(env, "memo.deleted", &memo).await;
            } else {
                db(env)?.prepare("UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id = ?")
                    .bind(&[js_num(unix_now()), js_num(memo.id)])?
                    .run()
                    .await?;
                let archived = DbMemo { row_status: "ARCHIVED".to_string(), updated_ts: unix_now(), ..memo.clone() };
                emit_memo_event(env, "memo.archived", &archived).await;
            }
            json_response(json!({ "ok": true }), 200).map_err(AppError::from)
        }
        _ => Err(AppError::new(405, "Method not allowed")),
    }
}

fn memo_child_route<'a>(parts: &'a [&'a str], method: &Method) -> MemoChildRoute<'a> {
    match (parts.get(1).copied(), method) {
        (Some("comments"), Method::Get) => MemoChildRoute::ListComments,
        (Some("comments"), Method::Post) => MemoChildRoute::CreateComment,
        (Some("reactions"), Method::Get) => MemoChildRoute::ListReactions,
        (Some("reactions"), Method::Post) => MemoChildRoute::UpsertReaction,
        (Some("reactions"), Method::Delete) if parts.len() > 2 => MemoChildRoute::DeleteReaction(parts[2]),
        (Some("relations"), Method::Get) if parts.len() == 2 => MemoChildRoute::GetRelations,
        (Some("relations"), Method::Post) if parts.get(2) == Some(&"suggest") => MemoChildRoute::SuggestRelations,
        (Some("relations"), Method::Patch) if parts.len() == 2 => MemoChildRoute::SetRelations,
        (Some("shares"), Method::Get) => MemoChildRoute::ListShares,
        (Some("shares"), Method::Post) => MemoChildRoute::CreateShare,
        (Some("shares"), Method::Delete) if parts.len() > 2 => MemoChildRoute::DeleteShare(parts[2]),
        _ => MemoChildRoute::Unsupported,
    }
}

async fn update_memo(req: &mut Request, env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_write(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let content = body.get("content").and_then(Value::as_str).unwrap_or(&memo.content).trim().to_string();
    let visibility = normalize_visibility(body.get("visibility").and_then(Value::as_str).unwrap_or(&memo.visibility))?;
    let row_status = normalize_state(body.get("rowStatus").or_else(|| body.get("row_status")).and_then(Value::as_str).unwrap_or(&memo.row_status))?;
    let pinned = body.get("pinned").and_then(Value::as_bool).map(|value| if value { 1 } else { 0 }).unwrap_or(memo.pinned);
    let payload = build_memo_payload(&content);
    db(env)?.prepare("UPDATE memo SET updated_ts = ?, content = ?, visibility = ?, pinned = ?, row_status = ?, payload = ? WHERE id = ?")
        .bind(&[js_num(unix_now()), content.into(), visibility.into(), js_num(pinned), row_status.into(), payload.to_string().into(), js_num(memo.id)])?
        .run()
        .await?;
    let updated = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(500, "Memo disappeared"))?;
    let event_type = if memo.row_status != updated.row_status {
        if updated.row_status == "ARCHIVED" { "memo.archived" } else { "memo.restored" }
    } else {
        "memo.updated"
    };
    emit_memo_event(env, event_type, &updated).await;
    let updated = memo_with_attachments(env, updated).await?;
    json_response(json!({ "memo": updated }), 200).map_err(AppError::from)
}

async fn bulk_memos(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let action = body.get("action").and_then(Value::as_str).unwrap_or("").to_uppercase();
    let mut seen = BTreeSet::new();
    let uids: Vec<String> = body.get("memoUids").and_then(Value::as_array).unwrap_or(&Vec::new())
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|uid| !uid.is_empty())
        .filter(|uid| seen.insert(uid.to_string()))
        .map(ToString::to_string)
        .take(200)
        .collect();
    if uids.is_empty() {
        return Err(AppError::new(400, "memoUids is required"));
    }
    let memos = get_memos_by_uids(env, viewer, &uids).await?;
    let ids: Vec<i64> = memos.iter().map(|memo| memo.id).collect();
    let result = json!({ "updated": 0, "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) });
    if ids.is_empty() {
        return json_response(result, 200).map_err(AppError::from);
    }
    let placeholders = placeholders(ids.len());
    let now = unix_now();
    match action.as_str() {
        "ARCHIVE" => {
            db(env)?.prepare(format!("UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id IN ({})", placeholders))
                .bind(&bind_with_first(now, &ids))?
                .run()
                .await?;
            emit_bulk_memo_events(env, &memos, "ARCHIVE", ids.len(), 0, uids.len().saturating_sub(ids.len()), now, Some("ARCHIVED"), None).await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "RESTORE" => {
            db(env)?.prepare(format!("UPDATE memo SET row_status = 'NORMAL', updated_ts = ? WHERE id IN ({})", placeholders))
                .bind(&bind_with_first(now, &ids))?
                .run()
                .await?;
            emit_bulk_memo_events(env, &memos, "RESTORE", ids.len(), 0, uids.len().saturating_sub(ids.len()), now, Some("NORMAL"), None).await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "DELETE" => {
            purge_ids(env, &ids).await?;
            emit_bulk_memo_events(env, &memos, "DELETE", 0, ids.len(), uids.len().saturating_sub(ids.len()), now, None, None).await;
            json_response(json!({ "updated": 0, "deleted": ids.len(), "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "VISIBILITY" => {
            let visibility = normalize_visibility(body.get("visibility").and_then(Value::as_str).unwrap_or(""))?;
            let mut values = vec![visibility.clone().into(), js_num(now)];
            values.extend(ids.iter().map(|id| js_num(*id)));
            db(env)?.prepare(format!("UPDATE memo SET visibility = ?, updated_ts = ? WHERE id IN ({})", placeholders))
                .bind(&values)?
                .run()
                .await?;
            emit_bulk_memo_events(env, &memos, "VISIBILITY", ids.len(), 0, uids.len().saturating_sub(ids.len()), now, None, Some(&visibility)).await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        _ => Err(AppError::new(400, "Invalid bulk action")),
    }
}

async fn export_data(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let users = db(env)?.prepare("SELECT id, username, role, email, nickname, avatar_url, description, row_status FROM \"user\" ORDER BY id")
        .all()
        .await?;
    let memos = db(env)?.prepare("SELECT * FROM memo ORDER BY created_ts, id")
        .all()
        .await?;
    let attachments = db(env)?.prepare("SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload FROM attachment ORDER BY created_ts, id")
        .all()
        .await?;
    let users: Vec<Value> = users.results()?;
    let memos: Vec<Value> = memos.results()?;
    let attachments: Vec<Value> = attachments.results()?;
    json_response(json!({
        "exportedAt": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default(),
        "users": users,
        "memos": memos,
        "attachments": attachments
    }), 200).map_err(AppError::from)
}

async fn import_data(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let mut imported = 0;
    let now = unix_now();
    if let Some(memos) = body.get("memos").and_then(Value::as_array) {
        for item in memos {
            let content = item.get("content").and_then(Value::as_str).unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            let uid = item.get("uid").and_then(Value::as_str).map(ToString::to_string).unwrap_or_else(|| generate_uid("m"));
            let created_ts = item.get("created_ts").or_else(|| item.get("createdTs")).and_then(Value::as_i64).unwrap_or(now);
            let updated_ts = item.get("updated_ts").or_else(|| item.get("updatedTs")).and_then(Value::as_i64).unwrap_or(created_ts);
            let row_status = normalize_state(item.get("row_status").or_else(|| item.get("rowStatus")).and_then(Value::as_str).unwrap_or("NORMAL"))?;
            let visibility = normalize_visibility(item.get("visibility").and_then(Value::as_str).unwrap_or("PRIVATE"))?;
            let pinned = item.get("pinned").and_then(Value::as_bool).map(|value| if value { 1 } else { 0 }).or_else(|| item.get("pinned").and_then(Value::as_i64)).unwrap_or(0);
            let payload = item.get("payload").cloned().unwrap_or_else(|| build_memo_payload(content));
            db(env)?.prepare("INSERT OR IGNORE INTO memo (uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&[
                    uid.into(),
                    js_num(viewer.id),
                    js_num(created_ts),
                    js_num(updated_ts),
                    row_status.into(),
                    content.into(),
                    visibility.into(),
                    js_num(pinned),
                    payload.to_string().into(),
                ])?
                .run()
                .await?;
            imported += 1;
        }
    }
    json_response(json!({ "imported": imported }), 200).map_err(AppError::from)
}

async fn public_share(env: &Env, uid: &str) -> std::result::Result<Response, AppError> {
    let row: Option<DbMemo> = db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo_share JOIN memo ON memo.id = memo_share.memo_id JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo_share.uid = ? AND memo.row_status = 'NORMAL' AND (memo_share.expires_ts IS NULL OR memo_share.expires_ts > ?)")
        .bind(&[uid.into(), js_num(unix_now())])?
        .first(None)
        .await?;
    let memo = row.ok_or_else(|| AppError::new(404, "Share not found"))?;
    let mut memo = memo_with_attachments(env, memo).await?;
    memo.attachments = memo.attachments.into_iter().map(|attachment| {
        let attachment_uid = attachment.get("uid").and_then(Value::as_str).unwrap_or("").to_string();
        let filename = attachment.get("filename").and_then(Value::as_str).unwrap_or("attachment").to_string();
        let mut next = attachment;
        if let Some(object) = next.as_object_mut() {
            object.insert("url".to_string(), shared_attachment_url(uid, &attachment_uid, &filename).into());
        }
        next
    }).collect();
    json_response(json!({ "memo": memo }), 200).map_err(AppError::from)
}

async fn download_shared_attachment(env: &Env, share_uid: &str, attachment_uid: &str) -> std::result::Result<Response, AppError> {
    let attachment: Option<DbAttachment> = db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment JOIN memo_share ON memo_share.memo_id = attachment.memo_id JOIN memo ON memo.id = memo_share.memo_id WHERE memo_share.uid = ? AND attachment.uid = ? AND memo.row_status = 'NORMAL' AND (memo_share.expires_ts IS NULL OR memo_share.expires_ts > ?)")
        .bind(&[share_uid.into(), attachment_uid.into(), js_num(unix_now())])?
        .first(None)
        .await?;
    let attachment = attachment.ok_or_else(|| AppError::new(404, "Attachment not found"))?;
    let object = env.bucket("MEMOS_BUCKET")?.get(attachment.reference.clone()).execute().await?
        .ok_or_else(|| AppError::new(404, "File not found"))?;
    let body = object.body().ok_or_else(|| AppError::new(404, "File not found"))?.response_body()?;
    let mut response = ResponseBuilder::new().body(body);
    response.headers_mut().set("Content-Type", if attachment.file_type.is_empty() { "application/octet-stream" } else { &attachment.file_type })?;
    response.headers_mut().set("Content-Disposition", &format!("inline; filename=\"{}\"", attachment.filename))?;
    response.headers_mut().set("Cache-Control", "public, max-age=3600")?;
    Ok(response)
}

async fn generate_rss(env: &Env, username: Option<&str>) -> std::result::Result<Response, AppError> {
    let rows = if let Some(username) = username {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.row_status = 'NORMAL' AND memo.visibility = 'PUBLIC' AND \"user\".username = ? ORDER BY memo.created_ts DESC LIMIT 50")
            .bind(&[username.into()])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.row_status = 'NORMAL' AND memo.visibility = 'PUBLIC' ORDER BY memo.created_ts DESC LIMIT 50")
            .all()
            .await?
    };
    let memos: Vec<DbMemo> = rows.results()?;
    let items = memos.into_iter().map(|memo| {
        let title = extract_title(&memo.content);
        format!(
            "    <item>\n      <title>{}</title>\n      <link>/memos/{}</link>\n      <guid isPermaLink=\"false\">memos/{}</guid>\n      <pubDate>{}</pubDate>\n      <author>{}</author>\n      <description>{}</description>\n    </item>",
            escape_xml(&title),
            escape_xml(&memo.uid),
            escape_xml(&memo.uid),
            js_sys::Date::new(&JsValue::from_f64((memo.created_ts * 1000) as f64)).to_utc_string().as_string().unwrap_or_default(),
            escape_xml(&memo.creator_username),
            escape_xml(&memo.content),
        )
    }).collect::<Vec<_>>().join("\n");
    let self_path = username.map(|name| format!("/api/v1/u/{}/rss.xml", name)).unwrap_or_else(|| "/api/v1/explore/rss.xml".to_string());
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n  <channel>\n    <title>Memos Worker</title>\n    <link>/</link>\n    <description>A lightweight memo hub</description>\n    <lastBuildDate>{}</lastBuildDate>\n    <atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\"/>\n{}\n  </channel>\n</rss>",
        js_sys::Date::new_0().to_utc_string().as_string().unwrap_or_default(),
        escape_xml(&self_path),
        items
    );
    let mut response = Response::ok(xml)?;
    response.headers_mut().set("Content-Type", "application/rss+xml; charset=utf-8")?;
    response.headers_mut().set("Cache-Control", "public, max-age=600")?;
    Ok(response)
}

async fn migration_preview(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let (memos, truncated) = fetch_original_memos(&options).await?;
    let preview = summarize_original_memos(&memos, truncated);
    let _ = env;
    json_response(json!({ "preview": preview }), 200).map_err(AppError::from)
}

async fn migration_import(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let progress = import_original_memos(env, viewer, &options, None).await?;
    record_migration_audit(env, viewer, &options, &progress).await;
    json_response(json!({ "result": progress }), 200).map_err(AppError::from)
}

async fn migration_import_stream(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let env = env.clone();
    let viewer = viewer.clone();
    let (sender, receiver) = mpsc::unbounded::<Vec<u8>>();
    wasm_bindgen_futures::spawn_local(async move {
        let mut sender = sender;
        record_migration_start_audit(&env, &viewer, &options).await;
        let result = import_original_memos_streaming(&env, &viewer, &options, |event, progress| {
            send_sse_chunk(&mut sender, event, progress)
        }).await;
        match result {
            Ok(progress) => {
                record_migration_audit(&env, &viewer, &options, &progress).await;
                let _ = send_sse_chunk(&mut sender, "done", &progress);
            }
            Err(error) => {
                record_migration_error_audit(&env, &viewer, &options, &error.message).await;
                let _ = send_sse_chunk(&mut sender, "error", &json!({ "error": error.message }));
            }
        }
    });

    let stream = receiver.map(|chunk| Ok::<Vec<u8>, worker::Error>(chunk));
    let mut response = Response::from_stream(stream)?;
    response.headers_mut().set("Content-Type", "text/event-stream; charset=utf-8")?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    response.headers_mut().set("X-Accel-Buffering", "no")?;
    Ok(response)
}

async fn read_migration_request(req: &mut Request) -> std::result::Result<MigrationOptions, AppError> {
    let body: MigrationRequest = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let raw = body.base_url.unwrap_or_default();
    let mut base_url = Url::parse(raw.trim()).map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
    if base_url.scheme() != "http" && base_url.scheme() != "https" {
        return Err(AppError::new(400, "Only http and https URLs are supported"));
    }
    base_url.set_query(None);
    base_url.set_fragment(None);
    let mut base = base_url.to_string();
    while base.ends_with('/') {
        base.pop();
    }
    let access_token = body.access_token.unwrap_or_default().trim().to_string();
    if access_token.is_empty() {
        return Err(AppError::new(400, "Access token is required"));
    }
    Ok(MigrationOptions {
        base_url: base,
        access_token,
        include_archived: body.include_archived.unwrap_or(false),
    })
}

async fn fetch_original_memos(options: &MigrationOptions) -> std::result::Result<(Vec<OriginalMemo>, bool), AppError> {
    let mut all = Vec::new();
    let mut truncated = false;
    let states = if options.include_archived { vec!["NORMAL", "ARCHIVED"] } else { vec!["NORMAL"] };
    for state in states {
        let mut page_token = String::new();
        loop {
            let mut url = Url::parse(&format!("{}/api/v1/memos", options.base_url)).map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
            url.query_pairs_mut()
                .append_pair("pageSize", &MIGRATION_PAGE_SIZE.to_string())
                .append_pair("state", state);
            if !page_token.is_empty() {
                url.query_pairs_mut().append_pair("pageToken", &page_token);
            }
            let (memos, next_page_token) = fetch_original_memos_page(options, url.as_str()).await?;
            for memo in memos {
                if all.len() >= MIGRATION_MAX_MEMOS {
                    truncated = true;
                    break;
                }
                all.push(memo);
            }
            if truncated || next_page_token.is_empty() {
                break;
            }
            page_token = next_page_token;
        }
        if truncated {
            break;
        }
    }
    Ok((all, truncated))
}

async fn fetch_original_memos_page(options: &MigrationOptions, url: &str) -> std::result::Result<(Vec<OriginalMemo>, String), AppError> {
    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", options.access_token))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(url, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(400, format!("Original Memos API returned HTTP {}", response.status_code())));
    }
    let data: Value = response.json().await?;
    let memos = data.get("memos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value::<OriginalMemo>(value).ok())
        .collect();
    let next = data.get("nextPageToken").and_then(Value::as_str).unwrap_or("").to_string();
    Ok((memos, next))
}

async fn import_original_memos(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    mut events: Option<&mut Vec<String>>,
) -> std::result::Result<MigrationProgress, AppError> {
    let (memos, truncated) = fetch_original_memos(options).await?;
    let summary = summarize_original_memos(&memos, truncated);
    let mut progress = MigrationProgress {
        phase: "importing".to_string(),
        processed: 0,
        imported: 0,
        skipped: 0,
        memo_count: summary.memo_count,
        attachment_count: summary.attachment_count,
        relation_count: summary.relation_count,
        archived_count: summary.archived_count,
        truncated,
        state: None,
    };
    if let Some(buf) = events.as_deref_mut() {
        buf.push(sse_event("progress", &progress)?);
    }
    for memo in memos {
        if import_single_original_memo(env, viewer, &memo).await? {
            progress.imported += 1;
        } else {
            progress.skipped += 1;
        }
        progress.processed += 1;
        if let Some(buf) = events.as_deref_mut() {
            buf.push(sse_event("progress", &progress)?);
        }
    }
    progress.phase = "done".to_string();
    if let Some(buf) = events.as_deref_mut() {
        buf.push(sse_event("progress", &progress)?);
    }
    Ok(progress)
}

async fn import_original_memos_streaming<F>(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    mut on_progress: F,
) -> std::result::Result<MigrationProgress, AppError>
where
    F: FnMut(&str, &MigrationProgress) -> std::result::Result<(), AppError>,
{
    let states = if options.include_archived { vec!["NORMAL", "ARCHIVED"] } else { vec!["NORMAL"] };
    let mut progress = MigrationProgress {
        phase: "fetching".to_string(),
        processed: 0,
        imported: 0,
        skipped: 0,
        memo_count: 0,
        attachment_count: 0,
        relation_count: 0,
        archived_count: 0,
        truncated: false,
        state: None,
    };
    on_progress("progress", &progress)?;
    let mut imported_original_names = BTreeSet::new();

    for state in states {
        let mut page_token = String::new();
        loop {
            let previous_page_token = page_token.clone();
            progress.phase = "fetching".to_string();
            progress.state = Some(state.to_string());
            on_progress("progress", &progress)?;

            let mut url = Url::parse(&format!("{}/api/v1/memos", options.base_url)).map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
            url.query_pairs_mut()
                .append_pair("pageSize", &MIGRATION_PAGE_SIZE.to_string())
                .append_pair("state", state);
            if !page_token.is_empty() {
                url.query_pairs_mut().append_pair("pageToken", &page_token);
            }

            let (memos, next_page_token) = fetch_original_memos_page(options, url.as_str()).await?;
            let existing_names = existing_imported_original_names(env, viewer.id, &memos).await?;
            progress.phase = "importing".to_string();
            for memo in memos {
                if progress.memo_count >= MIGRATION_MAX_MEMOS {
                    progress.truncated = true;
                    break;
                }
                progress.memo_count += 1;
                progress.attachment_count += memo.attachments.as_ref().map(Vec::len).unwrap_or(0);
                progress.relation_count += memo.relations.as_ref().map(Vec::len).unwrap_or(0);
                if normalize_original_state(memo.state.as_deref()) == "ARCHIVED" {
                    progress.archived_count += 1;
                }
                let original_name = original_memo_name(&memo);
                let already_imported = !original_name.is_empty()
                    && (existing_names.contains(&original_name) || imported_original_names.contains(&original_name));
                if import_single_original_memo_inner(env, viewer, &memo, already_imported).await? {
                    if !original_name.is_empty() {
                        imported_original_names.insert(original_name);
                    }
                    progress.imported += 1;
                } else {
                    progress.skipped += 1;
                }
                progress.processed += 1;
                on_progress("progress", &progress)?;
            }

            if progress.truncated || next_page_token.is_empty() {
                break;
            }
            if next_page_token == previous_page_token {
                return Err(AppError::new(400, "Original Memos API returned a repeated page token"));
            }
            page_token = next_page_token;
        }
        if progress.truncated {
            break;
        }
    }

    progress.phase = "done".to_string();
    on_progress("progress", &progress)?;
    Ok(progress)
}

async fn import_single_original_memo(env: &Env, viewer: &Viewer, memo: &OriginalMemo) -> std::result::Result<bool, AppError> {
    let original_name = original_memo_name(memo);
    let already_imported = !original_name.is_empty() && has_imported_original_memo(env, viewer.id, &original_name).await?;
    import_single_original_memo_inner(env, viewer, memo, already_imported).await
}

async fn import_single_original_memo_inner(env: &Env, viewer: &Viewer, memo: &OriginalMemo, already_imported: bool) -> std::result::Result<bool, AppError> {
    let content = memo.content.as_deref().unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Ok(false);
    }
    let original_name = original_memo_name(memo);
    if already_imported {
        return Ok(false);
    }
    let now = unix_now();
    let created_ts = parse_original_timestamp(memo.create_time.as_ref(), now);
    let updated_ts = parse_original_timestamp(memo.update_time.as_ref(), created_ts);
    let uid = generate_uid("m");
    let attachments = memo.attachments.clone().unwrap_or_default();
    let relations = memo.relations.clone().unwrap_or_default();
    let mut payload = build_memo_payload_with_tags(&content, memo.tags.as_ref());
    payload["source"] = json!({
        "type": "usememos",
        "originalName": original_name,
        "creator": memo.creator.as_deref().unwrap_or(""),
        "attachmentCount": attachments.len(),
        "relationCount": relations.len(),
        "attachments": attachments,
        "relations": relations
    });
    if let Some(property) = &memo.property {
        payload["originalProperty"] = property.clone();
    }
    if let Some(parent) = &memo.parent {
        payload["originalParent"] = json!(parent);
    }
    if let Some(location) = &memo.location {
        payload["originalLocation"] = location.clone();
    }
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&[
            uid.into(),
            js_num(viewer.id),
            js_num(created_ts),
            js_num(updated_ts),
            normalize_original_state(memo.state.as_deref()).into(),
            content.into(),
            normalize_original_visibility(memo.visibility.as_deref()).into(),
            js_num(if memo.pinned.unwrap_or(false) { 1 } else { 0 }),
            payload.to_string().into(),
        ])?
        .run()
        .await?;
    Ok(true)
}

async fn existing_imported_original_names(env: &Env, creator_id: i64, memos: &[OriginalMemo]) -> std::result::Result<BTreeSet<String>, AppError> {
    let names: BTreeSet<String> = memos.iter()
        .map(original_memo_name)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return Ok(BTreeSet::new());
    }
    let mut existing = BTreeSet::new();
    let names: Vec<String> = names.into_iter().collect();
    for chunk in names.chunks(SQL_IN_CHUNK_SIZE) {
        let mut values = vec![js_num(creator_id)];
        values.extend(chunk.iter().map(|name| name.clone().into()));
        let rows = db(env)?.prepare(format!(
            "SELECT json_extract(payload, '$.source.originalName') AS original_name FROM memo WHERE creator_id = ? AND json_extract(payload, '$.source.type') = 'usememos' AND json_extract(payload, '$.source.originalName') IN ({})",
            placeholders(chunk.len())
        ))
            .bind(&values)?
            .all()
            .await?;
        let values: Vec<Value> = rows.results()?;
        existing.extend(values.into_iter()
            .filter_map(|row| row.get("original_name").and_then(Value::as_str).map(ToString::to_string)));
    }
    Ok(existing)
}

async fn has_imported_original_memo(env: &Env, creator_id: i64, original_name: &str) -> std::result::Result<bool, AppError> {
    let row: Option<i64> = db(env)?.prepare("SELECT id FROM memo WHERE creator_id = ? AND json_extract(payload, '$.source.type') = 'usememos' AND json_extract(payload, '$.source.originalName') = ? LIMIT 1")
        .bind(&[js_num(creator_id), original_name.into()])?
        .first(Some("id"))
        .await?;
    Ok(row.is_some())
}

fn original_memo_name(memo: &OriginalMemo) -> String {
    memo.name.as_deref().unwrap_or("").trim().to_string()
}

fn summarize_original_memos(memos: &[OriginalMemo], truncated: bool) -> MigrationSummary {
    let mut summary = MigrationSummary {
        memo_count: 0,
        attachment_count: 0,
        relation_count: 0,
        archived_count: 0,
        truncated,
    };
    for memo in memos {
        summary.memo_count += 1;
        summary.attachment_count += memo.attachments.as_ref().map(Vec::len).unwrap_or(0);
        summary.relation_count += memo.relations.as_ref().map(Vec::len).unwrap_or(0);
        if normalize_original_state(memo.state.as_deref()) == "ARCHIVED" {
            summary.archived_count += 1;
        }
    }
    summary
}

fn build_memo_payload_with_tags(content: &str, original_tags: Option<&Vec<String>>) -> Value {
    let mut payload = build_memo_payload(content);
    if let Some(tags) = original_tags {
        if let Some(existing) = payload.get_mut("tags").and_then(Value::as_array_mut) {
            for tag in tags.iter().map(|tag| tag.trim()).filter(|tag| !tag.is_empty()) {
                if !existing.iter().any(|value| value.as_str() == Some(tag)) {
                    existing.push(json!(tag));
                }
            }
        }
    }
    payload
}

fn parse_original_timestamp(value: Option<&Value>, fallback: i64) -> i64 {
    match value {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(fallback),
        Some(Value::String(text)) if !text.trim().is_empty() => {
            let parsed = js_sys::Date::parse(text);
            if parsed.is_finite() { (parsed / 1000.0).floor() as i64 } else { fallback }
        }
        _ => fallback,
    }
}

fn normalize_original_state(value: Option<&str>) -> String {
    let state = value.unwrap_or("NORMAL").to_ascii_uppercase().replace("STATE_", "");
    match state.as_str() {
        "" | "UNSPECIFIED" => "NORMAL".to_string(),
        "DELETED" => "ARCHIVED".to_string(),
        "ARCHIVED" => "ARCHIVED".to_string(),
        _ => "NORMAL".to_string(),
    }
}

fn normalize_original_visibility(value: Option<&str>) -> String {
    let visibility = value.unwrap_or("PRIVATE").to_ascii_uppercase().replace("VISIBILITY_", "");
    match visibility.as_str() {
        "PUBLIC" | "PROTECTED" | "PRIVATE" => visibility,
        _ => "PRIVATE".to_string(),
    }
}

fn sse_event<T: Serialize>(event: &str, data: &T) -> std::result::Result<String, AppError> {
    let data = serde_json::to_string(data).map_err(|error| AppError::new(500, error.to_string()))?;
    Ok(format!("event: {}\ndata: {}\n\n", event, data))
}

fn send_sse_chunk<T: Serialize>(sender: &mut mpsc::UnboundedSender<Vec<u8>>, event: &str, data: &T) -> std::result::Result<(), AppError> {
    let chunk = sse_event(event, data)?;
    sender.unbounded_send(chunk.into_bytes()).map_err(|_| AppError::new(500, "Migration progress stream closed"))
}

async fn connect_sse(req: &Request, env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let last_event_id = req.headers().get("Last-Event-ID").ok().flatten();
    let since_id = sse_since_id(last_event_id.as_deref(), url);
    let body = sse_connection_payload(env, viewer, since_id).await?;
    let mut response = Response::ok(body)?;
    response.headers_mut().set("Content-Type", "text/event-stream; charset=utf-8")?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    response.headers_mut().set("X-Accel-Buffering", "no")?;
    Ok(response)
}

fn sse_ready_payload(user_id: i64) -> std::result::Result<String, AppError> {
    Ok(format!("retry: 5000\n{}", sse_event("ready", &json!({ "userId": user_id }))?))
}

async fn sse_connection_payload(env: &Env, viewer: &Viewer, since_id: Option<i64>) -> std::result::Result<String, AppError> {
    let mut body = sse_ready_payload(viewer.id)?;
    for event in list_memo_events(env, viewer, since_id).await? {
        body.push_str(&memo_event_sse(&event)?);
    }
    Ok(body)
}

fn sse_since_id(last_event_id: Option<&str>, url: &Url) -> Option<i64> {
    last_event_id
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or_else(|| query_param(url, "since").and_then(|value| value.parse::<i64>().ok()))
        .filter(|value| *value > 0)
}

fn memo_event_sse(event: &DbMemoEvent) -> std::result::Result<String, AppError> {
    let mut payload = serde_json::from_str::<Value>(&event.payload).unwrap_or_else(|_| json!({}));
    if let Some(object) = payload.as_object_mut() {
        object.insert("id".to_string(), json!(event.id.to_string()));
        object.insert("type".to_string(), json!(event.event_type.clone()));
        object.insert("name".to_string(), json!(event.name.clone()));
        object.insert("visibility".to_string(), json!(event.visibility.clone()));
        object.insert("creatorId".to_string(), json!(event.creator_id));
    }
    format_sse_event(Some(event.id), Some(&event.event_type), &payload)
}

fn format_sse_event(id: Option<i64>, event: Option<&str>, data: &Value) -> std::result::Result<String, AppError> {
    let mut chunk = String::new();
    if let Some(id) = id {
        chunk.push_str(&format!("id: {}\n", id));
    }
    if let Some(event) = event {
        chunk.push_str(&format!("event: {}\n", event));
    }
    chunk.push_str(&format!("data: {}\n\n", serde_json::to_string(data).map_err(|error| AppError::new(500, error.to_string()))?));
    Ok(chunk)
}

async fn list_memo_events(env: &Env, viewer: &Viewer, since_id: Option<i64>) -> std::result::Result<Vec<DbMemoEvent>, AppError> {
    ensure_memo_event_table(env).await?;
    let rows = if let Some(id) = since_id {
        db(env)?.prepare("SELECT * FROM memo_event WHERE id > ? AND (? = 'ADMIN' OR visibility != 'PRIVATE' OR creator_id = ?) ORDER BY id ASC LIMIT 100")
            .bind(&[js_num(id), viewer.role.clone().into(), js_num(viewer.id)])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT * FROM memo_event WHERE created_ts >= ? AND (? = 'ADMIN' OR visibility != 'PRIVATE' OR creator_id = ?) ORDER BY id ASC LIMIT 100")
            .bind(&[js_num(unix_now() - 60), viewer.role.clone().into(), js_num(viewer.id)])?
            .all()
            .await?
    };
    Ok(rows.results()?)
}

async fn emit_memo_event(env: &Env, event_type: &str, memo: &DbMemo) {
    emit_memo_change(env, event_type, memo, json!({})).await;
}

async fn emit_memo_change(env: &Env, event_type: &str, memo: &DbMemo, detail: Value) {
    if let Err(error) = record_memo_event_with_detail(env, event_type, memo, detail.clone()).await {
        console_log!("memo event record failed: {}", error.message);
    }
    if let Err(error) = fire_memo_webhooks(env, event_type, memo, detail).await {
        console_log!("memo webhook dispatch failed: {}", error.message);
    }
}

async fn record_memo_event_with_detail(env: &Env, event_type: &str, memo: &DbMemo, detail: Value) -> std::result::Result<(), AppError> {
    ensure_memo_event_table(env).await?;
    let payload = memo_event_payload_with_detail(event_type, memo, detail);
    db(env)?.prepare("INSERT INTO memo_event (created_ts, event_type, name, visibility, creator_id, payload) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&[
            js_num(unix_now()),
            event_type.into(),
            format!("memos/{}", memo.uid).into(),
            memo.visibility.clone().into(),
            js_num(memo.creator_id),
            payload.to_string().into(),
        ])?
        .run()
        .await?;
    Ok(())
}

async fn emit_bulk_memo_events(
    env: &Env,
    memos: &[DbMemo],
    action: &str,
    updated: usize,
    deleted: usize,
    skipped: usize,
    updated_ts: i64,
    row_status: Option<&str>,
    visibility: Option<&str>,
) {
    let detail = json!({
        "action": action,
        "updated": updated,
        "deleted": deleted,
        "skipped": skipped
    });
    for memo in memos {
        let event_memo = DbMemo {
            updated_ts,
            row_status: row_status.unwrap_or(&memo.row_status).to_string(),
            visibility: visibility.unwrap_or(&memo.visibility).to_string(),
            ..memo.clone()
        };
        emit_memo_change(env, "memo.bulk.updated", &event_memo, detail.clone()).await;
    }
}

fn memo_event_payload(event_type: &str, memo: &DbMemo) -> Value {
    json!({
        "type": event_type,
        "name": format!("memos/{}", memo.uid),
        "visibility": memo.visibility,
        "creatorId": memo.creator_id
    })
}

fn memo_event_payload_with_detail(event_type: &str, memo: &DbMemo, detail: Value) -> Value {
    let mut payload = memo_event_payload(event_type, memo);
    if let Value::Object(base) = &mut payload {
        match detail {
            Value::Object(extra) => {
                for (key, value) in extra {
                    base.insert(key, value);
                }
            }
            Value::Null => {}
            other => {
                base.insert("detail".to_string(), other);
            }
        }
    }
    payload
}

fn memo_webhook_body(event_type: &str, memo: &DbMemo, timestamp: i64, detail: Value) -> Value {
    json!({
        "event": event_type,
        "timestamp": timestamp,
        "payload": {
            "memo": public_memo(memo.clone()),
            "detail": detail
        }
    })
}

async fn fire_memo_webhooks(env: &Env, event_type: &str, memo: &DbMemo, detail: Value) -> std::result::Result<(), AppError> {
    let rows = db(env)?.prepare("SELECT * FROM webhook WHERE creator_id = ? AND row_status = 'NORMAL' ORDER BY id")
        .bind(&[js_num(memo.creator_id)])?
        .all()
        .await?;
    let webhooks: Vec<DbWebhook> = rows.results()?;
    if webhooks.is_empty() {
        return Ok(());
    }
    let body = memo_webhook_body(event_type, memo, unix_now(), detail).to_string();
    for webhook in webhooks {
        if let Err(error) = send_and_record_webhook(env, webhook.id, memo.creator_id, &webhook.url, event_type, &body).await {
            console_log!("webhook delivery failed: {}", error.message);
        }
    }
    Ok(())
}

async fn prune_memo_events(env: &Env, retention_days: i64) -> std::result::Result<i64, AppError> {
    ensure_memo_event_table(env).await?;
    let cutoff = memo_event_retention_cutoff(unix_now(), retention_days);
    db(env)?.prepare("DELETE FROM memo_event WHERE created_ts < ?")
        .bind(&[js_num(cutoff)])?
        .run()
        .await?;
    let count: Option<i64> = db(env)?.prepare("SELECT changes() AS count")
        .first(Some("count"))
        .await?;
    Ok(count.unwrap_or(0))
}

fn memo_event_retention_cutoff(now: i64, retention_days: i64) -> i64 {
    now - retention_days.max(0) * 24 * 60 * 60
}

async fn ensure_memo_event_table(env: &Env) -> std::result::Result<(), AppError> {
    db(env)?.prepare("CREATE TABLE IF NOT EXISTS memo_event (id INTEGER PRIMARY KEY AUTOINCREMENT, created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), event_type TEXT NOT NULL, name TEXT NOT NULL, visibility TEXT NOT NULL DEFAULT 'PRIVATE', creator_id INTEGER NOT NULL, payload TEXT NOT NULL DEFAULT '{}')")
        .run()
        .await?;
    db(env)?.prepare("CREATE INDEX IF NOT EXISTS idx_memo_event_created ON memo_event(created_ts, id)")
        .run()
        .await?;
    db(env)?.prepare("CREATE INDEX IF NOT EXISTS idx_memo_event_visibility ON memo_event(visibility, creator_id, id)")
        .run()
        .await?;
    Ok(())
}

async fn record_migration_audit(env: &Env, viewer: &Viewer, options: &MigrationOptions, progress: &MigrationProgress) {
    record_audit(env, Some(viewer), "migration.usememos.import", "usememos", json!({
        "baseUrl": options.base_url,
        "imported": progress.imported,
        "skipped": progress.skipped,
        "memoCount": progress.memo_count,
        "attachmentCount": progress.attachment_count,
        "relationCount": progress.relation_count,
        "archivedCount": progress.archived_count,
        "truncated": progress.truncated
    })).await;
}

async fn record_migration_start_audit(env: &Env, viewer: &Viewer, options: &MigrationOptions) {
    record_audit(env, Some(viewer), "migration.usememos.start", "usememos", json!({
        "baseUrl": options.base_url,
        "includeArchived": options.include_archived
    })).await;
}

async fn record_migration_error_audit(env: &Env, viewer: &Viewer, options: &MigrationOptions, message: &str) {
    record_audit(env, Some(viewer), "migration.usememos.error", "usememos", json!({
        "baseUrl": options.base_url,
        "error": message
    })).await;
}

async fn create_share(req: &mut Request, env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let expires_ts = body.get("expiresTs").and_then(Value::as_i64);
    let share_uid = generate_uid("s");
    let now = unix_now();
    db(env)?.prepare("INSERT INTO memo_share (uid, memo_id, creator_id, created_ts, expires_ts) VALUES (?, ?, ?, ?, ?)")
        .bind(&[
            share_uid.clone().into(),
            js_num(memo.id),
            js_num(viewer.id),
            js_num(now),
            expires_ts.map(js_num).unwrap_or(JsValue::NULL),
        ])?
        .run()
        .await?;
    emit_memo_change(env, "share.created", &memo, json!({ "shareUid": share_uid.clone(), "expiresTs": expires_ts })).await;
    json_response(json!({
        "share": {
            "uid": share_uid,
            "memoUid": memo.uid,
            "createdTs": now,
            "expiresTs": expires_ts,
            "url": format!("/api/v1/shares/{}", share_uid)
        }
    }), 201).map_err(AppError::from)
}

async fn list_shares(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let rows = db(env)?.prepare("SELECT * FROM memo_share WHERE memo_id = ? ORDER BY created_ts DESC")
        .bind(&[js_num(memo.id)])?
        .all()
        .await?;
    let shares: Vec<DbShare> = rows.results()?;
    let payload: Vec<Value> = shares.into_iter().map(|share| json!({
        "id": share.id,
        "uid": share.uid,
        "memoUid": memo.uid,
        "createdTs": share.created_ts,
        "expiresTs": share.expires_ts,
        "url": format!("/api/v1/shares/{}", share.uid)
    })).collect();
    json_response(json!({ "shares": payload }), 200).map_err(AppError::from)
}

async fn delete_share(env: &Env, viewer: &Viewer, uid: &str, share_id: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    let id = share_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid share ID"))?;
    let share: Option<DbShare> = db(env)?.prepare("SELECT * FROM memo_share WHERE id = ? AND memo_id = ?")
        .bind(&[js_num(id), js_num(memo.id)])?
        .first(None)
        .await?;
    let share = share.ok_or_else(|| AppError::new(404, "Share not found"))?;
    if viewer.role != "ADMIN" && share.creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    db(env)?.prepare("DELETE FROM memo_share WHERE id = ?")
        .bind(&[js_num(id)])?
        .run()
        .await?;
    emit_memo_change(env, "share.deleted", &memo, json!({ "shareId": share.id, "shareUid": share.uid })).await;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn get_ai_settings(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let settings = resolve_ai_settings(env).await?;
    json_response(json!({ "settings": public_ai_settings(&settings) }), 200).map_err(AppError::from)
}

async fn update_ai_settings(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let previous = read_stored_ai_settings(env).await?;
    let next = merge_ai_settings(&previous, &body)?;
    db(env)?.prepare("INSERT INTO system_setting (name, value, description) VALUES ('ai.settings', ?, 'AI model settings') ON CONFLICT(name) DO UPDATE SET value = excluded.value")
        .bind(&[serde_json::to_string(&next).map_err(|error| AppError::new(500, error.to_string()))?.into()])?
        .run()
        .await?;
    json_response(json!({ "settings": public_ai_settings(&next) }), 200).map_err(AppError::from)
}

async fn test_ai_settings(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let base = resolve_ai_settings(env).await?;
    let settings = merge_ai_settings(&base, &body)?;
    if settings.api_key.trim().is_empty() {
        return Err(AppError::new(400, "AI API Key is required"));
    }
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", settings.api_key))?;
    headers.set("Content-Type", "application/json")?;
    let payload = json!({
        "model": settings.model,
        "temperature": 0,
        "messages": [
            { "role": "system", "content": "Return ok." },
            { "role": "user", "content": "ping" }
        ],
        "max_tokens": 8
    });
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&payload.to_string())));
    let request = Request::new_with_init(&format!("{}/chat/completions", settings.base_url.trim_end_matches('/')), &init)?;
    let response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(502, format!("AI API returned HTTP {}", response.status_code())));
    }
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn resolve_ai_settings(env: &Env) -> std::result::Result<AiSettings, AppError> {
    let stored = read_stored_ai_settings(env).await?;
    Ok(AiSettings {
        base_url: normalize_http_url(
            if stored.base_url.is_empty() {
                env.var("AI_BASE_URL").map(|value| value.to_string()).unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
            } else {
                stored.base_url
            },
            "Invalid AI Base URL",
        )?,
        model: if stored.model.is_empty() {
            env.var("AI_MODEL").map(|value| value.to_string()).unwrap_or_else(|_| "gpt-4o-mini".to_string())
        } else {
            stored.model
        },
        api_key: if stored.api_key.is_empty() {
            env.secret("AI_API_KEY").map(|value| value.to_string()).unwrap_or_default()
        } else {
            stored.api_key
        },
    })
}

async fn read_stored_ai_settings(env: &Env) -> std::result::Result<AiSettings, AppError> {
    let value: Option<String> = db(env)?.prepare("SELECT value FROM system_setting WHERE name = 'ai.settings'")
        .first(Some("value"))
        .await?;
    let stored = value.and_then(|text| serde_json::from_str::<AiSettings>(&text).ok());
    Ok(stored.unwrap_or(AiSettings {
        base_url: "https://api.openai.com/v1".to_string(),
        model: "gpt-4o-mini".to_string(),
        api_key: String::new(),
    }))
}

fn merge_ai_settings(previous: &AiSettings, update: &Value) -> std::result::Result<AiSettings, AppError> {
    let base_url = update.get("baseUrl").and_then(Value::as_str).unwrap_or(&previous.base_url);
    let model = update.get("model").and_then(Value::as_str).unwrap_or(&previous.model).trim();
    let api_key = update.get("apiKey").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).unwrap_or(&previous.api_key);
    Ok(AiSettings {
        base_url: normalize_http_url(base_url, "Invalid AI Base URL")?,
        model: if model.is_empty() { "gpt-4o-mini".to_string() } else { model.to_string() },
        api_key: api_key.to_string(),
    })
}

fn public_ai_settings(settings: &AiSettings) -> Value {
    json!({
        "baseUrl": settings.base_url,
        "model": settings.model,
        "configured": !settings.api_key.trim().is_empty()
    })
}

async fn create_backup(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let artifact = create_backup_artifact(env).await?;
    record_audit(env, Some(viewer), "backup.create", &artifact.key, json!({ "size": artifact.size })).await;
    json_response(backup_artifact_payload(&artifact), 201).map_err(AppError::from)
}

async fn create_scheduled_backup(env: &Env) -> std::result::Result<BackupArtifact, AppError> {
    let artifact = create_backup_artifact(env).await?;
    record_audit(env, None, "backup.create", &artifact.key, json!({ "size": artifact.size, "source": "scheduled" })).await;
    Ok(artifact)
}

async fn create_backup_artifact(env: &Env) -> std::result::Result<BackupArtifact, AppError> {
    let key = backup_key();
    let body = serde_json::to_string_pretty(&build_backup_payload(env).await?).map_err(|error| AppError::new(500, error.to_string()))?;
    let size = body.as_bytes().len();
    env.bucket("MEMOS_BUCKET")?
        .put(key.clone(), body)
        .http_metadata(HttpMetadata {
            content_type: Some("application/json".to_string()),
            ..Default::default()
        })
        .execute()
        .await?;
    Ok(BackupArtifact { key, size })
}

fn backup_artifact_payload(artifact: &BackupArtifact) -> Value {
    json!({ "backup": { "key": artifact.key, "size": artifact.size } })
}

async fn list_backups(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let listed = env.bucket("MEMOS_BUCKET")?.list().prefix("backups/").execute().await?;
    let mut backups: Vec<Value> = listed.objects().into_iter().map(|object| json!({
        "key": object.key(),
        "size": object.size(),
        "uploaded": object.uploaded().to_string()
    })).collect();
    backups.sort_by(|a, b| b.get("uploaded").and_then(Value::as_str).cmp(&a.get("uploaded").and_then(Value::as_str)));
    json_response(json!({ "backups": backups }), 200).map_err(AppError::from)
}

async fn download_backup(env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let key = url.query_pairs().find(|(name, _)| name == "key").map(|(_, value)| value.to_string()).unwrap_or_default();
    if !key.starts_with("backups/") {
        return Err(AppError::new(400, "Invalid backup key"));
    }
    let object = env.bucket("MEMOS_BUCKET")?.get(key.clone()).execute().await?
        .ok_or_else(|| AppError::new(404, "Backup not found"))?;
    let body = object.body().ok_or_else(|| AppError::new(404, "Backup not found"))?.response_body()?;
    let filename = key.rsplit('/').next().unwrap_or("memos-backup.json");
    let mut response = ResponseBuilder::new().body(body);
    response.headers_mut().set("Content-Type", "application/json")?;
    response.headers_mut().set("Content-Disposition", &format!("attachment; filename=\"{}\"", filename))?;
    Ok(response)
}

async fn preview_backup(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let payload = read_backup_payload(req, env).await?;
    json_response(json!({ "preview": backup_preview(&payload) }), 200).map_err(AppError::from)
}

async fn restore_backup(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let payload = read_backup_payload(req, env).await?;
    let preview = backup_preview(&payload);
    if let Some(memos) = payload.get("memos").and_then(Value::as_array) {
        for item in memos {
            db(env)?.prepare("INSERT OR REPLACE INTO memo (id, uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&[
                    json_bind(item.get("id")),
                    json_bind(item.get("uid")),
                    json_bind(item.get("creator_id")),
                    json_bind(item.get("created_ts")),
                    json_bind(item.get("updated_ts")),
                    json_bind(item.get("row_status")).if_undefined("NORMAL"),
                    json_bind(item.get("content")).if_undefined(""),
                    json_bind(item.get("visibility")).if_undefined("PRIVATE"),
                    json_bind(item.get("pinned")),
                    json_bind(item.get("payload")).if_undefined("{}"),
                ])?
                .run()
                .await?;
        }
    }
    if let Some(attachments) = payload.get("attachments").and_then(Value::as_array) {
        for item in attachments {
            db(env)?.prepare("INSERT OR REPLACE INTO attachment (id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&[
                    json_bind(item.get("id")),
                    json_bind(item.get("uid")),
                    json_bind(item.get("creator_id")),
                    json_bind(item.get("created_ts")),
                    json_bind(item.get("updated_ts")),
                    json_bind(item.get("filename")).if_undefined("attachment"),
                    json_bind(item.get("type")).if_undefined(""),
                    json_bind(item.get("size")),
                    json_bind(item.get("memo_id")),
                    json_bind(item.get("storage_type")).if_undefined("S3"),
                    json_bind(item.get("reference")).if_undefined(""),
                    json_bind(item.get("payload")).if_undefined("{}"),
                ])?
                .run()
                .await?;
        }
    }
    db(env)?.prepare("DELETE FROM memo_relation").run().await?;
    if let Some(relations) = payload.get("relations").and_then(Value::as_array) {
        for item in relations {
            db(env)?.prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, ?)")
                .bind(&[
                    json_bind(item.get("memo_id")),
                    json_bind(item.get("related_memo_id")),
                    json_bind(item.get("type")).if_undefined("REFERENCE"),
                ])?
                .run()
                .await?;
        }
    }
    record_audit(env, Some(viewer), "backup.restore", "backup", preview.clone()).await;
    json_response(json!({ "restored": preview }), 200).map_err(AppError::from)
}

async fn read_backup_payload(req: &mut Request, env: &Env) -> std::result::Result<Value, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    if let Some(payload) = body.get("payload") {
        return Ok(payload.clone());
    }
    let key = body.get("key").and_then(Value::as_str).unwrap_or("");
    if !key.starts_with("backups/") {
        return Err(AppError::new(400, "Invalid backup key"));
    }
    let object = env.bucket("MEMOS_BUCKET")?.get(key.to_string()).execute().await?
        .ok_or_else(|| AppError::new(404, "Backup not found"))?;
    let text = object.body().ok_or_else(|| AppError::new(404, "Backup not found"))?.text().await?;
    serde_json::from_str(&text).map_err(|_| AppError::new(400, "Invalid backup payload"))
}

async fn build_backup_payload(env: &Env) -> std::result::Result<Value, AppError> {
    let users = db(env)?.prepare("SELECT id, created_ts, updated_ts, row_status, username, role, email, nickname, avatar_url, description FROM \"user\" ORDER BY id").all().await?.results::<Value>()?;
    let memos = db(env)?.prepare("SELECT * FROM memo ORDER BY created_ts, id").all().await?.results::<Value>()?;
    let attachments = db(env)?.prepare("SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload FROM attachment ORDER BY created_ts, id").all().await?.results::<Value>()?;
    let relations = db(env)?.prepare("SELECT * FROM memo_relation ORDER BY memo_id, related_memo_id").all().await?.results::<Value>()?;
    Ok(json!({
        "exportedAt": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default(),
        "users": users,
        "memos": memos,
        "attachments": attachments,
        "relations": relations
    }))
}

fn backup_preview(payload: &Value) -> Value {
    json!({
        "userCount": payload.get("users").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "memoCount": payload.get("memos").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "attachmentCount": payload.get("attachments").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "relationCount": payload.get("relations").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
    })
}

fn backup_key() -> String {
    let stamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_else(|| unix_now().to_string()).replace([':', '.'], "-");
    format!("backups/memos-{}.json", stamp)
}

async fn list_audit_logs(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    ensure_audit_log_table(env).await?;
    let rows = db(env)?.prepare("SELECT audit_log.*, \"user\".username AS actor_username FROM audit_log LEFT JOIN \"user\" ON \"user\".id = audit_log.actor_id ORDER BY audit_log.created_ts DESC, audit_log.id DESC LIMIT 100")
        .all()
        .await?;
    let logs: Vec<DbAuditLog> = rows.results()?;
    let payload: Vec<Value> = logs.into_iter().map(|row| json!({
        "id": row.id,
        "createdTs": row.created_ts,
        "actorId": row.actor_id,
        "actorUsername": row.actor_username,
        "action": row.action,
        "actionLabel": audit_action_label(&row.action),
        "target": row.target,
        "detail": serde_json::from_str::<Value>(&row.detail).unwrap_or_else(|_| json!({}))
    })).collect();
    json_response(json!({ "logs": payload }), 200).map_err(AppError::from)
}

async fn list_webhooks(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let rows = db(env)?.prepare("SELECT * FROM webhook WHERE creator_id = ? ORDER BY created_ts DESC")
        .bind(&[js_num(viewer.id)])?
        .all()
        .await?;
    let webhooks: Vec<DbWebhook> = rows.results()?;
    let payload: Vec<Value> = webhooks.into_iter().map(public_webhook).collect();
    json_response(json!({ "webhooks": payload }), 200).map_err(AppError::from)
}

async fn create_webhook(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let name = body.get("name").and_then(Value::as_str).unwrap_or("").trim();
    let url = normalize_http_url(body.get("url").and_then(Value::as_str).unwrap_or(""), "Invalid webhook URL")?;
    if name.is_empty() {
        return Err(AppError::new(400, "Name is required"));
    }
    let now = unix_now();
    db(env)?.prepare("INSERT INTO webhook (created_ts, updated_ts, creator_id, name, url) VALUES (?, ?, ?, ?, ?)")
        .bind(&[js_num(now), js_num(now), js_num(viewer.id), name.into(), url.clone().into()])?
        .run()
        .await?;
    let id: Option<i64> = db(env)?.prepare("SELECT id FROM webhook WHERE creator_id = ? AND name = ? AND url = ? ORDER BY id DESC LIMIT 1")
        .bind(&[js_num(viewer.id), name.into(), url.clone().into()])?
        .first(Some("id"))
        .await?;
    let id = id.unwrap_or(0);
    record_audit(env, Some(viewer), "webhook.create", &id.to_string(), json!({ "name": name })).await;
    json_response(json!({ "webhook": { "id": id, "name": name, "url": url, "rowStatus": "NORMAL", "createdTs": now, "updatedTs": now } }), 201).map_err(AppError::from)
}

async fn update_webhook(req: &mut Request, env: &Env, viewer: &Viewer, webhook_id: &str) -> std::result::Result<Response, AppError> {
    let id = webhook_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let existing: Option<DbWebhook> = db(env)?.prepare("SELECT * FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let existing = existing.ok_or_else(|| AppError::new(404, "Webhook not found"))?;
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let name = body.get("name").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).unwrap_or(&existing.name);
    let url = if let Some(value) = body.get("url").and_then(Value::as_str) {
        normalize_http_url(value, "Invalid webhook URL")?
    } else {
        existing.url
    };
    let row_status = body.get("rowStatus").and_then(Value::as_str).filter(|value| matches!(*value, "NORMAL" | "ARCHIVED")).unwrap_or(&existing.row_status);
    let now = unix_now();
    db(env)?.prepare("UPDATE webhook SET name = ?, url = ?, row_status = ?, updated_ts = ? WHERE id = ?")
        .bind(&[name.into(), url.clone().into(), row_status.into(), js_num(now), js_num(id)])?
        .run()
        .await?;
    json_response(json!({ "webhook": { "id": id, "name": name, "url": url, "rowStatus": row_status, "updatedTs": now } }), 200).map_err(AppError::from)
}

async fn delete_webhook(env: &Env, viewer: &Viewer, webhook_id: &str) -> std::result::Result<Response, AppError> {
    let id = webhook_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let existing: Option<i64> = db(env)?.prepare("SELECT id FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(Some("id"))
        .await?;
    if existing.is_none() {
        return Err(AppError::new(404, "Webhook not found"));
    }
    db(env)?.prepare("DELETE FROM webhook WHERE id = ?").bind(&[js_num(id)])?.run().await?;
    record_audit(env, Some(viewer), "webhook.delete", webhook_id, json!({})).await;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn list_webhook_deliveries(env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let webhook_id = url.query_pairs().find(|(key, _)| key == "webhookId").and_then(|(_, value)| value.parse::<i64>().ok());
    let rows = if let Some(id) = webhook_id.filter(|id| *id > 0) {
        db(env)?.prepare("SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url FROM webhook_delivery JOIN webhook ON webhook.id = webhook_delivery.webhook_id WHERE webhook_delivery.creator_id = ? AND webhook_delivery.webhook_id = ? ORDER BY webhook_delivery.created_ts DESC, webhook_delivery.id DESC LIMIT 50")
            .bind(&[js_num(viewer.id), js_num(id)])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url FROM webhook_delivery JOIN webhook ON webhook.id = webhook_delivery.webhook_id WHERE webhook_delivery.creator_id = ? ORDER BY webhook_delivery.created_ts DESC, webhook_delivery.id DESC LIMIT 50")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    };
    let deliveries: Vec<DbWebhookDelivery> = rows.results()?;
    let payload: Vec<Value> = deliveries.into_iter().map(public_webhook_delivery).collect();
    json_response(json!({ "deliveries": payload }), 200).map_err(AppError::from)
}

async fn test_webhook(env: &Env, viewer: &Viewer, webhook_id: &str) -> std::result::Result<Response, AppError> {
    let id = webhook_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let webhook: Option<DbWebhook> = db(env)?.prepare("SELECT * FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let webhook = webhook.ok_or_else(|| AppError::new(404, "Webhook not found"))?;
    let delivery = send_and_record_webhook(env, webhook.id, viewer.id, &webhook.url, "webhook.test", &json!({
        "event": "webhook.test",
        "timestamp": unix_now(),
        "payload": { "ok": true, "source": "memos-worker" }
    }).to_string()).await?;
    json_response(json!({ "delivery": delivery.map(public_webhook_delivery) }), 201).map_err(AppError::from)
}

async fn retry_webhook_delivery(env: &Env, viewer: &Viewer, delivery_id: &str) -> std::result::Result<Response, AppError> {
    let id = delivery_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid delivery ID"))?;
    let row: Option<DbWebhookDelivery> = db(env)?.prepare("SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url FROM webhook_delivery JOIN webhook ON webhook.id = webhook_delivery.webhook_id WHERE webhook_delivery.id = ? AND webhook_delivery.creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let row = row.ok_or_else(|| AppError::new(404, "Webhook delivery not found"))?;
    let url = row.webhook_url.as_deref().unwrap_or("");
    let delivery = send_and_record_webhook(env, row.webhook_id, viewer.id, url, &row.event, &row.request_body).await?;
    json_response(json!({ "delivery": delivery.map(public_webhook_delivery) }), 200).map_err(AppError::from)
}

async fn send_and_record_webhook(env: &Env, webhook_id: i64, creator_id: i64, url: &str, event: &str, request_body: &str) -> std::result::Result<Option<DbWebhookDelivery>, AppError> {
    let started = js_sys::Date::now();
    let mut status_code: Option<i64> = None;
    let mut response_body = String::new();
    let mut error = String::new();
    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(request_body)));
    match Request::new_with_init(url, &init) {
        Ok(request) => match Fetch::Request(request).send().await {
            Ok(mut response) => {
                let code = response.status_code() as i64;
                status_code = Some(code);
                response_body = response.text().await.unwrap_or_default();
                if !(200..300).contains(&code) {
                    error = format!("HTTP {}", code);
                }
            }
            Err(err) => error = err.to_string(),
        },
        Err(err) => error = err.to_string(),
    }
    let duration_ms = (js_sys::Date::now() - started).max(0.0).round() as i64;
    let status = if status_code.map(|code| (200..300).contains(&code)).unwrap_or(false) { "SUCCESS" } else { "FAILED" };
    db(env)?.prepare("INSERT INTO webhook_delivery (webhook_id, creator_id, created_ts, event, status, status_code, duration_ms, error, request_body, response_body) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&[
            js_num(webhook_id),
            js_num(creator_id),
            js_num(unix_now()),
            truncate(event, 200).into(),
            status.into(),
            status_code.map(js_num).unwrap_or(JsValue::NULL),
            js_num(duration_ms),
            truncate(&error, 1000).into(),
            truncate(request_body, 12000).into(),
            truncate(&response_body, 4000).into(),
        ])?
        .run()
        .await?;
    let inserted: Option<DbWebhookDelivery> = db(env)?.prepare("SELECT * FROM webhook_delivery WHERE webhook_id = ? AND creator_id = ? ORDER BY id DESC LIMIT 1")
        .bind(&[js_num(webhook_id), js_num(creator_id)])?
        .first(None)
        .await?;
    prune_webhook_deliveries(env, creator_id).await;
    Ok(inserted)
}

async fn prune_webhook_deliveries(env: &Env, creator_id: i64) {
    if let Ok(database) = db(env) {
        let stmt = database.prepare("DELETE FROM webhook_delivery WHERE creator_id = ? AND id NOT IN (SELECT id FROM webhook_delivery WHERE creator_id = ? ORDER BY created_ts DESC, id DESC LIMIT 200)");
        if let Ok(bound) = stmt.bind(&[js_num(creator_id), js_num(creator_id)]) {
            let _ = bound.run().await;
        }
    }
}

fn public_webhook(webhook: DbWebhook) -> Value {
    json!({
        "id": webhook.id,
        "name": webhook.name,
        "url": webhook.url,
        "rowStatus": webhook.row_status,
        "createdTs": webhook.created_ts,
        "updatedTs": webhook.updated_ts
    })
}

fn public_webhook_delivery(delivery: DbWebhookDelivery) -> Value {
    json!({
        "id": delivery.id,
        "webhookId": delivery.webhook_id,
        "webhookName": delivery.webhook_name.unwrap_or_default(),
        "webhookUrl": delivery.webhook_url.unwrap_or_default(),
        "createdTs": delivery.created_ts,
        "event": delivery.event,
        "status": delivery.status,
        "statusCode": delivery.status_code,
        "durationMs": delivery.duration_ms,
        "error": delivery.error,
        "responseBody": delivery.response_body
    })
}

async fn list_inbox(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let rows = db(env)?.prepare("SELECT inbox.*, \"user\".username AS sender_username, \"user\".nickname AS sender_nickname FROM inbox LEFT JOIN \"user\" ON \"user\".id = inbox.sender_id WHERE inbox.receiver_id = ? ORDER BY inbox.created_ts DESC LIMIT 100")
        .bind(&[js_num(viewer.id)])?
        .all()
        .await?;
    let inbox: Vec<DbInboxRow> = rows.results()?;
    let unread_count: Option<i64> = db(env)?.prepare("SELECT COUNT(*) AS count FROM inbox WHERE receiver_id = ? AND status = 'UNREAD'")
        .bind(&[js_num(viewer.id)])?
        .first(Some("count"))
        .await?;
    let payload: Vec<Value> = inbox.into_iter().map(public_inbox_item).collect();
    json_response(json!({ "inbox": payload, "unreadCount": unread_count.unwrap_or(0) }), 200).map_err(AppError::from)
}

async fn update_inbox_status(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let status = if body.get("status").and_then(Value::as_str) == Some("READ") { "READ" } else { "UNREAD" };
    let ids: Vec<i64> = body.get("ids")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_i64).filter(|id| *id > 0).collect())
        .unwrap_or_default();

    if ids.is_empty() {
        db(env)?.prepare("UPDATE inbox SET status = ? WHERE receiver_id = ?")
            .bind(&[status.into(), js_num(viewer.id)])?
            .run()
            .await?;
    } else {
        let mut values: Vec<JsValue> = vec![status.into()];
        values.extend(ids.iter().map(|id| js_num(*id)));
        values.push(js_num(viewer.id));
        db(env)?.prepare(format!("UPDATE inbox SET status = ? WHERE id IN ({}) AND receiver_id = ?", placeholders(ids.len())))
            .bind(&values)?
            .run()
            .await?;
    }

    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn delete_inbox_item(env: &Env, viewer: &Viewer, item_id: &str) -> std::result::Result<Response, AppError> {
    let id = item_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid inbox ID"))?;
    db(env)?.prepare("DELETE FROM inbox WHERE id = ? AND receiver_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

fn public_inbox_item(row: DbInboxRow) -> Value {
    let sender = row.sender_id
        .map(|id| json!({ "id": id, "username": row.sender_username, "nickname": row.sender_nickname }))
        .unwrap_or(Value::Null);
    json!({
        "id": row.id,
        "createdTs": row.created_ts,
        "sender": sender,
        "status": row.status,
        "message": safe_inbox_message(&row.message)
    })
}

async fn record_comment_inbox(env: &Env, sender_id: i64, receiver_id: i64, parent_uid: &str, comment_uid: &str) -> std::result::Result<(), AppError> {
    db(env)?.prepare("INSERT INTO inbox (created_ts, sender_id, receiver_id, status, message) VALUES (?, ?, ?, 'UNREAD', ?)")
        .bind(&[
            js_num(unix_now()),
            js_num(sender_id),
            js_num(receiver_id),
            comment_inbox_message(parent_uid, comment_uid).to_string().into(),
        ])?
        .run()
        .await?;
    Ok(())
}

fn comment_inbox_message(parent_uid: &str, comment_uid: &str) -> Value {
    json!({
        "type": "memo.comment.created",
        "memoUid": parent_uid,
        "commentUid": comment_uid
    })
}

fn safe_inbox_message(value: &str) -> Value {
    serde_json::from_str::<Value>(value).unwrap_or_else(|_| json!({ "type": "unknown" }))
}

async fn list_attachments(env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let unattached = url.query_pairs().any(|(key, value)| key == "unattached" && value == "true");
    let rows = if viewer.role == "ADMIN" && unattached {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL ORDER BY attachment.created_ts DESC LIMIT 100")
            .all()
            .await?
    } else if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id ORDER BY attachment.created_ts DESC LIMIT 100")
            .all()
            .await?
    } else if unattached {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.creator_id = ? ORDER BY attachment.created_ts DESC LIMIT 100")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.creator_id = ? ORDER BY attachment.created_ts DESC LIMIT 100")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    };
    let attachments: Vec<DbAttachment> = rows.results()?;
    let payload: Vec<Value> = attachments.into_iter().map(public_attachment).collect();
    json_response(json!({ "attachments": payload }), 200).map_err(AppError::from)
}

async fn upload_attachment(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let form = req.form_data().await.map_err(|_| AppError::new(400, "Invalid form data"))?;
    let file = match form.get("file") {
        Some(FormEntry::File(file)) => file,
        _ => return Err(AppError::new(400, "file is required")),
    };
    if file.size() > 25 * 1024 * 1024 {
        return Err(AppError::new(413, "file is too large"));
    }

    let memo_uid = form.get_field("memoUid").unwrap_or_default();
    let memo_id = if memo_uid.trim().is_empty() {
        None
    } else {
        let memo = get_memo_by_uid(env, memo_uid.trim()).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
        if !can_write(&memo, viewer) {
            return Err(AppError::new(403, "Forbidden"));
        }
        Some(memo.id)
    };

    let original_filename = file.name();
    let filename = sanitize_filename(if original_filename.is_empty() { "attachment" } else { &original_filename });
    let file_type = if file.type_().is_empty() { "application/octet-stream".to_string() } else { file.type_() };
    let uid = generate_uid("a");
    let key = attachment_storage_key(viewer.id, &uid, &filename);
    let bytes = file.bytes().await?;
    let size = bytes.len() as i64;
    let mut metadata = HashMap::new();
    metadata.insert("creatorId".to_string(), viewer.id.to_string());
    metadata.insert("originalFilename".to_string(), if original_filename.is_empty() { filename.clone() } else { original_filename });

    env.bucket("MEMOS_BUCKET")?
        .put(key.clone(), bytes)
        .http_metadata(HttpMetadata {
            content_type: Some(file_type.clone()),
            content_disposition: Some(format!("inline; filename=\"{}\"", filename)),
            ..Default::default()
        })
        .custom_metadata(metadata)
        .execute()
        .await?;

    let now = unix_now();
    db(env)?.prepare("INSERT INTO attachment (uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'S3', ?, '{}')")
        .bind(&[
            uid.clone().into(),
            js_num(viewer.id),
            js_num(now),
            js_num(now),
            filename.into(),
            file_type.into(),
            js_num(size),
            memo_id.map(js_num).unwrap_or(JsValue::NULL),
            key.into(),
        ])?
        .run()
        .await?;

    let attachment = get_attachment_by_uid(env, &uid).await?.ok_or_else(|| AppError::new(500, "Failed to create attachment"))?;
    json_response(json!({ "attachment": public_attachment(attachment) }), 201).map_err(AppError::from)
}

async fn download_attachment(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let attachment = get_attachment_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Attachment not found"))?;
    if !can_read_attachment(&attachment, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let object = env.bucket("MEMOS_BUCKET")?.get(attachment.reference.clone()).execute().await?
        .ok_or_else(|| AppError::new(404, "File not found"))?;
    let body = object.body().ok_or_else(|| AppError::new(404, "File not found"))?.response_body()?;
    let mut response = ResponseBuilder::new().body(body);
    response.headers_mut().set("Content-Type", if attachment.file_type.is_empty() { "application/octet-stream" } else { &attachment.file_type })?;
    response.headers_mut().set("Content-Disposition", &format!("inline; filename=\"{}\"", attachment.filename))?;
    response.headers_mut().set("Cache-Control", if attachment.memo_visibility.as_deref() == Some("PUBLIC") { "public, max-age=3600" } else { "private, no-store" })?;
    Ok(response)
}

async fn delete_attachment(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let attachment = get_attachment_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Attachment not found"))?;
    if viewer.role != "ADMIN" && attachment.creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    if attachment.memo_id.is_some() {
        return Err(AppError::new(409, "Only unattached attachments can be deleted"));
    }
    let _ = env.bucket("MEMOS_BUCKET")?.delete(attachment.reference.clone()).await;
    db(env)?.prepare("DELETE FROM attachment WHERE id = ?")
        .bind(&[js_num(attachment.id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn batch_delete_attachments(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let uids: Vec<String> = body.get("attachmentUids")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::trim).filter(|uid| !uid.is_empty()).map(ToString::to_string).take(100).collect())
        .unwrap_or_default();
    let rows = if uids.is_empty() && viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL LIMIT 100")
            .all()
            .await?
    } else if uids.is_empty() {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.creator_id = ? LIMIT 100")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    } else {
        let placeholders = placeholders(uids.len());
        let mut values: Vec<JsValue> = uids.iter().map(|uid| uid.clone().into()).collect();
        let owner_sql = if viewer.role == "ADMIN" { "" } else { values.push(js_num(viewer.id)); " AND attachment.creator_id = ?" };
        db(env)?.prepare(format!("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.uid IN ({}){}", placeholders, owner_sql))
            .bind(&values)?
            .all()
            .await?
    };
    let attachments: Vec<DbAttachment> = rows.results()?;
    let mut deleted = 0;
    let mut size = 0;
    for attachment in attachments {
        let _ = env.bucket("MEMOS_BUCKET")?.delete(attachment.reference.clone()).await;
        db(env)?.prepare("DELETE FROM attachment WHERE id = ?")
            .bind(&[js_num(attachment.id)])?
            .run()
            .await?;
        deleted += 1;
        size += attachment.size;
    }
    json_response(json!({ "deleted": deleted, "size": size }), 200).map_err(AppError::from)
}

async fn get_attachment_by_uid(env: &Env, uid: &str) -> std::result::Result<Option<DbAttachment>, AppError> {
    Ok(db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.uid = ?")
        .bind(&[uid.into()])?
        .first(None)
        .await?)
}

fn public_attachment(attachment: DbAttachment) -> Value {
    json!({
        "name": format!("attachments/{}", attachment.uid),
        "uid": attachment.uid,
        "filename": attachment.filename,
        "type": attachment.file_type,
        "size": attachment.size,
        "memoId": attachment.memo_id,
        "createdTs": attachment.created_ts,
        "url": format!("/file/attachments/{}/{}", attachment.uid, attachment.filename)
    })
}

fn attachment_storage_key(creator_id: i64, uid: &str, filename: &str) -> String {
    format!("attachments/{}/{}/{}", creator_id, uid, filename)
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| if matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_control() { '_' } else { ch })
        .collect::<String>()
        .trim()
        .chars()
        .take(180)
        .collect();
    if cleaned.is_empty() || !cleaned.chars().any(char::is_alphanumeric) {
        "attachment".to_string()
    } else {
        cleaned
    }
}

fn can_read_attachment(attachment: &DbAttachment, viewer: &Viewer) -> bool {
    viewer.role == "ADMIN"
        || (attachment.memo_id.is_none() && attachment.creator_id == viewer.id)
        || attachment.memo_visibility.as_deref() != Some("PRIVATE")
        || attachment.memo_creator_id == Some(viewer.id)
}

async fn list_comments(env: &Env, viewer: &Viewer, parent_uid: &str) -> std::result::Result<Response, AppError> {
    let parent = get_memo_by_uid(env, parent_uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&parent, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let rows = db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN memo_relation ON memo_relation.related_memo_id = memo.id JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo_relation.memo_id = ? AND memo_relation.type = 'COMMENT' AND memo.row_status = 'NORMAL' ORDER BY memo.created_ts ASC")
        .bind(&[js_num(parent.id)])?
        .all()
        .await?;
    let memos: Vec<DbMemo> = rows.results()?;
    let public: Vec<PublicMemo> = memos.into_iter().map(public_memo).collect();
    json_response(json!({ "memos": public }), 200).map_err(AppError::from)
}

async fn create_comment(req: &mut Request, env: &Env, viewer: &Viewer, parent_uid: &str) -> std::result::Result<Response, AppError> {
    let parent = get_memo_by_uid(env, parent_uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&parent, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let content = body.get("content").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Err(AppError::new(400, "Content is required"));
    }
    let uid = generate_uid("m");
    let now = unix_now();
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, payload) VALUES (?, ?, ?, ?, ?, 'PROTECTED', ?)")
        .bind(&[uid.clone().into(), js_num(viewer.id), js_num(now), js_num(now), content.into(), build_memo_payload("").to_string().into()])?
        .run()
        .await?;
    let comment = get_memo_by_uid(env, &uid).await?.ok_or_else(|| AppError::new(500, "Failed to create comment"))?;
    db(env)?.prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'COMMENT')")
        .bind(&[js_num(parent.id), js_num(comment.id)])?
        .run()
        .await?;
    if let Err(error) = record_comment_inbox(env, viewer.id, parent.creator_id, &parent.uid, &comment.uid).await {
        console_log!("comment inbox record failed: {}", error.message);
    }
    emit_memo_change(env, "memo.created", &comment, json!({ "parentMemoUid": parent.uid.clone() })).await;
    emit_memo_change(env, "memo.comment.created", &parent, json!({ "comment": public_memo(comment.clone()) })).await;
    let comment = memo_with_attachments(env, comment).await?;
    json_response(json!({ "memo": comment }), 201).map_err(AppError::from)
}

async fn list_reactions(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    list_reactions_for_memo(env, memo.id).await
}

async fn upsert_reaction(req: &mut Request, env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let reaction_type = body.get("reactionType").and_then(Value::as_str).unwrap_or("").trim();
    if reaction_type.is_empty() {
        return Err(AppError::new(400, "reactionType is required"));
    }
    db(env)?.prepare("INSERT INTO reaction (created_ts, creator_id, content_type, content_id, reaction_type) VALUES (?, ?, 'MEMO', ?, ?) ON CONFLICT (creator_id, content_type, content_id, reaction_type) DO NOTHING")
        .bind(&[js_num(unix_now()), js_num(viewer.id), js_num(memo.id), reaction_type.into()])?
        .run()
        .await?;
    emit_memo_change(env, "reaction.upserted", &memo, json!({ "reactionType": reaction_type, "actorId": viewer.id })).await;
    list_reactions_for_memo(env, memo.id).await
}

async fn delete_reaction(env: &Env, viewer: &Viewer, uid: &str, reaction_id: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    let id = reaction_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid reaction ID"))?;
    let row: Option<Value> = db(env)?.prepare("SELECT id, creator_id FROM reaction WHERE id = ? AND content_type = 'MEMO' AND content_id = ?")
        .bind(&[js_num(id), js_num(memo.id)])?
        .first(None)
        .await?;
    let row = row.ok_or_else(|| AppError::new(404, "Reaction not found"))?;
    let creator_id = row.get("creator_id").and_then(Value::as_i64).unwrap_or_default();
    if viewer.role != "ADMIN" && creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    db(env)?.prepare("DELETE FROM reaction WHERE id = ?")
        .bind(&[js_num(id)])?
        .run()
        .await?;
    emit_memo_change(env, "reaction.deleted", &memo, json!({ "reactionId": id, "actorId": viewer.id })).await;
    list_reactions_for_memo(env, memo.id).await
}

async fn list_reactions_for_memo(env: &Env, memo_id: i64) -> std::result::Result<Response, AppError> {
    let rows = db(env)?.prepare("SELECT reaction.id, reaction.created_ts, reaction.reaction_type, reaction.creator_id, \"user\".username AS creator_username FROM reaction JOIN \"user\" ON \"user\".id = reaction.creator_id WHERE reaction.content_type = 'MEMO' AND reaction.content_id = ? ORDER BY reaction.created_ts ASC")
        .bind(&[js_num(memo_id)])?
        .all()
        .await?;
    let reactions: Vec<DbReaction> = rows.results()?;
    let payload: Vec<Value> = reactions.into_iter().map(|reaction| json!({
        "id": reaction.id,
        "reactionType": reaction.reaction_type,
        "creator": { "id": reaction.creator_id, "username": reaction.creator_username },
        "createdTs": reaction.created_ts
    })).collect();
    json_response(json!({ "reactions": payload }), 200).map_err(AppError::from)
}

async fn get_relations(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let refs = db(env)?.prepare("SELECT memo.uid, memo.content, memo_relation.type FROM memo_relation JOIN memo ON memo.id = memo_relation.related_memo_id WHERE memo_relation.memo_id = ? AND memo_relation.type = 'REFERENCE'")
        .bind(&[js_num(memo.id)])?
        .all()
        .await?;
    let back_refs = db(env)?.prepare("SELECT memo.uid, memo.content, memo_relation.type FROM memo_relation JOIN memo ON memo.id = memo_relation.memo_id WHERE memo_relation.related_memo_id = ? AND memo_relation.type = 'REFERENCE'")
        .bind(&[js_num(memo.id)])?
        .all()
        .await?;
    let outgoing: Vec<DbMemoRelation> = refs.results()?;
    let incoming: Vec<DbMemoRelation> = back_refs.results()?;
    let relations: Vec<Value> = outgoing.into_iter()
        .map(|rel| json!({ "memo": format!("memos/{}", rel.uid), "type": rel.relation_type, "direction": "outgoing", "content": rel.content }))
        .chain(incoming.into_iter().map(|rel| json!({ "memo": format!("memos/{}", rel.uid), "type": rel.relation_type, "direction": "incoming", "content": rel.content })))
        .collect();
    json_response(json!({ "relations": relations }), 200).map_err(AppError::from)
}

async fn set_relations(req: &mut Request, env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_write(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    db(env)?.prepare("DELETE FROM memo_relation WHERE memo_id = ? AND type = 'REFERENCE'")
        .bind(&[js_num(memo.id)])?
        .run()
        .await?;
    if let Some(relations) = body.get("relations").and_then(Value::as_array) {
        for rel in relations {
            let related_uid = rel.get("memo").and_then(Value::as_str).unwrap_or("").trim().trim_start_matches("memos/");
            if related_uid.is_empty() || related_uid == uid {
                continue;
            }
            if let Some(related) = get_memo_by_uid(env, related_uid).await? {
                db(env)?.prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'REFERENCE')")
                    .bind(&[js_num(memo.id), js_num(related.id)])?
                    .run()
                    .await?;
            }
        }
    }
    emit_memo_change(env, "memo.updated", &memo, json!({ "relationsUpdated": true })).await;
    get_relations(env, viewer, uid).await
}

async fn suggest_memo_relations(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    const RECENT_CANDIDATE_LIMIT: i64 = 80;
    const AI_CANDIDATE_LIMIT: usize = 30;

    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }

    let rows = db(env)?.prepare("SELECT memo.uid, memo.content, memo.payload, memo.updated_ts FROM memo WHERE memo.row_status = 'NORMAL' AND memo.id != ? AND (memo.visibility != 'PRIVATE' OR memo.creator_id = ? OR ? = 'ADMIN') AND NOT EXISTS (SELECT 1 FROM memo_relation WHERE memo_relation.memo_id = ? AND memo_relation.related_memo_id = memo.id AND memo_relation.type = 'REFERENCE') ORDER BY memo.updated_ts DESC, memo.id DESC LIMIT ?")
        .bind(&[js_num(memo.id), js_num(viewer.id), viewer.role.clone().into(), js_num(memo.id), js_num(RECENT_CANDIDATE_LIMIT)])?
        .all()
        .await?;
    let candidates: Vec<RelationCandidate> = rows.results()?;
    let ranked = rank_relation_candidates(&relation_candidate_from_memo(&memo), &candidates, AI_CANDIDATE_LIMIT);
    if ranked.is_empty() {
        return json_response(json!({ "suggestions": [] }), 200).map_err(AppError::from);
    }

    let candidate_content: HashMap<String, String> = ranked.iter()
        .map(|candidate| (candidate.uid.clone(), candidate.content.clone()))
        .collect();
    let settings = resolve_ai_settings(env).await?;
    let ai_suggestions = if settings.api_key.trim().is_empty() {
        Vec::new()
    } else {
        request_ai_relation_suggestions(&settings, &memo, &ranked, &candidate_content).await.unwrap_or_default()
    };
    let suggestions = if ai_suggestions.is_empty() {
        ranked.iter().take(5).map(|candidate| json!({
            "memo": format!("memos/{}", candidate.uid),
            "content": candidate.content,
            "reason": "标签或关键词相近",
            "confidence": (candidate.score / 10.0).clamp(0.35, 0.75),
            "source": "local"
        })).collect()
    } else {
        ai_suggestions
    };

    json_response(json!({ "suggestions": suggestions }), 200).map_err(AppError::from)
}

async fn request_ai_relation_suggestions(
    settings: &AiSettings,
    memo: &DbMemo,
    candidates: &[RankedRelationCandidate],
    candidate_content: &HashMap<String, String>,
) -> std::result::Result<Vec<Value>, AppError> {
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", settings.api_key))?;
    headers.set("Content-Type", "application/json")?;
    let payload = json!({
        "model": settings.model,
        "temperature": 0.1,
        "response_format": { "type": "json_object" },
        "messages": [
            {
                "role": "system",
                "content": "你是个人知识库的关联识别助手。只返回 JSON，不要解释。"
            },
            {
                "role": "user",
                "content": json!({
                    "task": "从 candidates 中选择最多 8 条和 currentMemo 最相关的笔记。返回 {\"suggestions\":[{\"memo\":\"memos/<uid>\",\"reason\":\"简短原因\",\"confidence\":0.0到1.0}]}。",
                    "currentMemo": {
                        "memo": format!("memos/{}", memo.uid),
                        "content": truncate(&memo.content, 1200)
                    },
                    "candidates": candidates.iter().map(|candidate| json!({
                        "memo": format!("memos/{}", candidate.uid),
                        "content": truncate(&candidate.content, 600),
                        "tags": candidate.tags
                    })).collect::<Vec<_>>()
                }).to_string()
            }
        ]
    });
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&payload.to_string())));
    let request = Request::new_with_init(&format!("{}/chat/completions", settings.base_url.trim_end_matches('/')), &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(502, format!("AI API returned HTTP {}", response.status_code())));
    }
    let data: Value = response.json().await?;
    let content = data.get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(parse_ai_relation_suggestions(content, candidate_content))
}

fn relation_candidate_from_memo(memo: &DbMemo) -> RelationCandidate {
    RelationCandidate {
        uid: memo.uid.clone(),
        content: memo.content.clone(),
        payload: memo.payload.clone(),
        updated_ts: memo.updated_ts,
    }
}

fn rank_relation_candidates(current: &RelationCandidate, candidates: &[RelationCandidate], limit: usize) -> Vec<RankedRelationCandidate> {
    let current_tags = extract_payload_tags(&current.payload);
    let current_keywords = extract_keywords(&current.content);
    let max_updated = candidates.iter().map(|candidate| candidate.updated_ts).chain(std::iter::once(current.updated_ts)).max().unwrap_or(1).max(1);
    let mut ranked: Vec<RankedRelationCandidate> = candidates.iter()
        .filter(|candidate| candidate.uid != current.uid && !candidate.content.trim().is_empty())
        .filter_map(|candidate| {
            let tags = extract_payload_tags(&candidate.payload);
            let keywords = extract_keywords(&candidate.content);
            let shared_tags = tags.iter().filter(|tag| current_tags.contains(*tag)).count() as f64;
            let shared_keywords = keywords.iter().filter(|keyword| current_keywords.contains(*keyword)).count() as f64;
            let recency = (candidate.updated_ts as f64 / max_updated as f64).clamp(0.0, 1.0);
            let score = shared_tags * 5.0 + shared_keywords * 2.0 + recency;
            if score > 0.0 {
                Some(RankedRelationCandidate {
                    uid: candidate.uid.clone(),
                    content: candidate.content.clone(),
                    score,
                    tags,
                })
            } else {
                None
            }
        })
        .collect();
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(limit);
    ranked
}

fn parse_ai_relation_suggestions(raw: &str, candidate_content: &HashMap<String, String>) -> Vec<Value> {
    const SUGGESTION_LIMIT: usize = 8;

    let parsed = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!({}));
    let list = parsed.get("suggestions").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut seen = BTreeSet::new();
    let mut suggestions = Vec::new();
    for item in list {
        let uid = item.get("memo")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim_start_matches("memos/")
            .trim()
            .to_string();
        let Some(content) = candidate_content.get(&uid) else {
            continue;
        };
        if uid.is_empty() || !seen.insert(uid.clone()) {
            continue;
        }
        let reason = truncate(item.get("reason").and_then(Value::as_str).unwrap_or("内容相关"), 160);
        let confidence = item.get("confidence").and_then(Value::as_f64).map(clamp_confidence).unwrap_or(0.5);
        suggestions.push(json!({
            "memo": format!("memos/{}", uid),
            "content": content,
            "reason": reason,
            "confidence": confidence,
            "source": "ai"
        }));
        if suggestions.len() >= SUGGESTION_LIMIT {
            break;
        }
    }
    suggestions
}

fn extract_payload_tags(payload: &str) -> Vec<String> {
    let parsed = serde_json::from_str::<Value>(payload).unwrap_or_else(|_| json!({}));
    parsed.get("tags")
        .and_then(Value::as_array)
        .map(|tags| tags.iter().filter_map(Value::as_str).map(str::trim).filter(|tag| !tag.is_empty()).map(ToString::to_string).collect())
        .unwrap_or_default()
}

fn extract_keywords(content: &str) -> Vec<String> {
    let mut words = BTreeSet::new();
    let mut current = String::new();
    for ch in content.to_lowercase().chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            current.push(ch);
        } else {
            push_keyword(&mut words, &mut current);
        }
    }
    push_keyword(&mut words, &mut current);
    words.into_iter().take(80).collect()
}

fn push_keyword(words: &mut BTreeSet<String>, current: &mut String) {
    let len = current.chars().count();
    if (2..=32).contains(&len) {
        words.insert(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn clamp_confidence(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

async fn update_me(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    db(env)?.prepare("UPDATE \"user\" SET updated_ts = ?, email = ?, nickname = ?, description = ?, avatar_url = ? WHERE id = ?")
        .bind(&[
            js_num(unix_now()),
            body.get("email").and_then(Value::as_str).unwrap_or("").trim().into(),
            body.get("nickname").and_then(Value::as_str).unwrap_or("").trim().into(),
            body.get("description").and_then(Value::as_str).unwrap_or("").trim().into(),
            body.get("avatarUrl").and_then(Value::as_str).unwrap_or("").trim().into(),
            js_num(viewer.id),
        ])?
        .run()
        .await?;
    let user = get_user_by_id(env, viewer.id).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    json_response(json!({ "user": public_user(user) }), 200).map_err(AppError::from)
}

async fn change_password(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let current = body.get("currentPassword").and_then(Value::as_str).ok_or_else(|| AppError::new(400, "Current password is required"))?;
    let new_password = body.get("newPassword").and_then(Value::as_str).ok_or_else(|| AppError::new(400, "Password is required"))?;
    assert_password(new_password)?;
    let user = get_user_by_id(env, viewer.id).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if !verify_password(current, &user.password_hash) {
        return Err(AppError::new(401, "Current password is incorrect"));
    }
    db(env)?.prepare("UPDATE \"user\" SET password_hash = ?, updated_ts = ? WHERE id = ?")
        .bind(&[hash_password(new_password).into(), js_num(unix_now()), js_num(viewer.id)])?
        .run()
        .await?;
    sign_out()
}

async fn list_users(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let rows = db(env)?.prepare("SELECT * FROM \"user\" ORDER BY id")
        .all()
        .await?;
    let users: Vec<DbUser> = rows.results()?;
    let payload: Vec<PublicUser> = users.into_iter().map(public_user).collect();
    json_response(json!({ "users": payload }), 200).map_err(AppError::from)
}

async fn user_subroute(req: &mut Request, env: &Env, viewer: &Viewer, identifier: &str, method: Method) -> std::result::Result<Response, AppError> {
    if let Some((user_identifier, key)) = parse_user_settings_path(identifier) {
        return match (key, method) {
            (None, Method::Get) => list_user_settings(env, viewer, user_identifier).await,
            (Some(setting_key), Method::Get) => get_user_setting(env, viewer, user_identifier, setting_key).await,
            (Some(setting_key), Method::Patch) => update_user_setting(req, env, viewer, user_identifier, setting_key).await,
            _ => Err(AppError::new(405, "Method not allowed")),
        };
    }

    match method {
        Method::Get => {
            let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
            if viewer.role != "ADMIN" && viewer.id != user.id {
                return Err(AppError::new(403, "Forbidden"));
            }
            json_response(json!({ "user": public_user(user) }), 200).map_err(AppError::from)
        }
        Method::Patch => update_user(req, env, viewer, identifier).await,
        Method::Delete => delete_user(env, viewer, identifier).await,
        _ => Err(AppError::new(405, "Method not allowed")),
    }
}

fn parse_user_settings_path(path: &str) -> Option<(&str, Option<&str>)> {
    let (identifier, rest) = path.split_once("/settings")?;
    if identifier.is_empty() {
        return None;
    }
    let key = rest.strip_prefix('/').filter(|value| !value.is_empty());
    if rest.is_empty() || key.is_some() {
        Some((identifier, key))
    } else {
        None
    }
}

async fn update_user(req: &mut Request, env: &Env, viewer: &Viewer, identifier: &str) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let role = if viewer.role == "ADMIN" {
        body.get("role").and_then(Value::as_str).filter(|role| matches!(*role, "ADMIN" | "USER")).unwrap_or(&user.role)
    } else {
        &user.role
    };
    let row_status = body.get("rowStatus").and_then(Value::as_str).filter(|status| matches!(*status, "NORMAL" | "ARCHIVED")).unwrap_or(&user.row_status);
    db(env)?.prepare("UPDATE \"user\" SET updated_ts = ?, email = ?, nickname = ?, description = ?, avatar_url = ?, role = ?, row_status = ? WHERE id = ?")
        .bind(&[
            js_num(unix_now()),
            body.get("email").and_then(Value::as_str).unwrap_or(&user.email).trim().into(),
            body.get("nickname").and_then(Value::as_str).unwrap_or(&user.nickname).trim().into(),
            body.get("description").and_then(Value::as_str).unwrap_or(&user.description).trim().into(),
            body.get("avatarUrl").and_then(Value::as_str).unwrap_or(&user.avatar_url).trim().into(),
            role.into(),
            row_status.into(),
            js_num(user.id),
        ])?
        .run()
        .await?;
    let updated = get_user_by_id(env, user.id).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    json_response(json!({ "user": public_user(updated) }), 200).map_err(AppError::from)
}

async fn delete_user(env: &Env, viewer: &Viewer, identifier: &str) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if user.id == viewer.id {
        return Err(AppError::new(400, "Cannot delete yourself"));
    }
    db(env)?.prepare("DELETE FROM \"user\" WHERE id = ?")
        .bind(&[js_num(user.id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn get_user_setting(env: &Env, viewer: &Viewer, identifier: &str, key: &str) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let value: Option<String> = db(env)?.prepare("SELECT value FROM user_setting WHERE user_id = ? AND key = ?")
        .bind(&[js_num(user.id), key.into()])?
        .first(Some("value"))
        .await?;
    json_response(json!({ "key": key, "value": value.unwrap_or_default() }), 200).map_err(AppError::from)
}

async fn list_user_settings(env: &Env, viewer: &Viewer, identifier: &str) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let rows = db(env)?.prepare("SELECT key, value FROM user_setting WHERE user_id = ?")
        .bind(&[js_num(user.id)])?
        .all()
        .await?;
    let settings: Vec<Value> = rows.results()?;
    json_response(json!({ "settings": settings }), 200).map_err(AppError::from)
}

async fn update_user_setting(req: &mut Request, env: &Env, viewer: &Viewer, identifier: &str, key: &str) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let value = body.get("value").and_then(Value::as_str).unwrap_or("").to_string();
    db(env)?.prepare("INSERT INTO user_setting (user_id, key, value) VALUES (?, ?, ?) ON CONFLICT (user_id, key) DO UPDATE SET value = excluded.value")
        .bind(&[js_num(user.id), key.into(), value.clone().into()])?
        .run()
        .await?;
    json_response(json!({ "key": key, "value": value }), 200).map_err(AppError::from)
}

async fn user_stats(env: &Env, viewer: &Viewer, identifier: &str) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let count: Option<i64> = db(env)?.prepare("SELECT COUNT(*) AS count FROM memo WHERE creator_id = ? AND row_status = 'NORMAL'")
        .bind(&[js_num(user.id)])?
        .first(Some("count"))
        .await?;
    let attachment_count: Option<i64> = db(env)?.prepare("SELECT COUNT(*) AS count FROM attachment WHERE creator_id = ?")
        .bind(&[js_num(user.id)])?
        .first(Some("count"))
        .await?;
    json_response(json!({ "stats": { "memoCount": count.unwrap_or(0), "attachmentCount": attachment_count.unwrap_or(0) } }), 200).map_err(AppError::from)
}

async fn list_access_tokens(env: &Env, viewer: &Viewer, identifier: &str) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let rows = db(env)?.prepare("SELECT id, name, token_prefix, created_ts, updated_ts, last_used_ts, expires_ts, row_status FROM user_access_token WHERE user_id = ? ORDER BY created_ts DESC")
        .bind(&[js_num(user.id)])?
        .all()
        .await?;
    let tokens: Vec<DbAccessToken> = rows.results()?;
    let payload: Vec<Value> = tokens.into_iter().map(|token| json!({
        "id": token.id,
        "name": token.name,
        "prefix": token.token_prefix,
        "createdTs": token.created_ts,
        "updatedTs": token.updated_ts,
        "lastUsedTs": token.last_used_ts,
        "expiresTs": token.expires_ts,
        "rowStatus": token.row_status
    })).collect();
    json_response(json!({ "accessTokens": payload }), 200).map_err(AppError::from)
}

async fn create_access_token(req: &mut Request, env: &Env, viewer: &Viewer, identifier: &str) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let name = body.get("name").and_then(Value::as_str).unwrap_or("Unnamed Token").trim();
    let expires_ts = body.get("expiresTs").and_then(Value::as_i64);
    let raw_token = format!("memos_pat_{}", base64url(&random_bytes(18)));
    let prefix: String = raw_token.chars().take(20).collect();
    let token_hash = sha256_hex(&raw_token);
    let now = unix_now();
    db(env)?.prepare("INSERT INTO user_access_token (user_id, name, token_prefix, token_hash, created_ts, updated_ts, expires_ts) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&[
            js_num(user.id),
            (if name.is_empty() { "Unnamed Token" } else { name }).into(),
            prefix.clone().into(),
            token_hash.clone().into(),
            js_num(now),
            js_num(now),
            expires_ts.map(js_num).unwrap_or(JsValue::NULL),
        ])?
        .run()
        .await?;
    let id: Option<i64> = db(env)?.prepare("SELECT id FROM user_access_token WHERE token_hash = ?")
        .bind(&[token_hash.into()])?
        .first(Some("id"))
        .await?;
    json_response(json!({
        "accessToken": {
            "id": id.unwrap_or(0),
            "name": if name.is_empty() { "Unnamed Token" } else { name },
            "token": raw_token,
            "prefix": prefix,
            "createdTs": now,
            "expiresTs": expires_ts
        }
    }), 201).map_err(AppError::from)
}

async fn delete_access_token(env: &Env, viewer: &Viewer, identifier: &str, token_id: &str) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier).await?.ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let id = token_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid token ID"))?;
    let row: Option<Value> = db(env)?.prepare("SELECT id FROM user_access_token WHERE id = ? AND user_id = ?")
        .bind(&[js_num(id), js_num(user.id)])?
        .first(None)
        .await?;
    if row.is_none() {
        return Err(AppError::new(404, "Token not found"));
    }
    db(env)?.prepare("DELETE FROM user_access_token WHERE id = ?")
        .bind(&[js_num(id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn list_tags(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let memos = get_recent_memos(env, viewer, 500).await?;
    let mut counts: HashMap<String, i64> = HashMap::new();
    for memo in memos {
        if let Ok(payload) = serde_json::from_str::<Value>(&memo.payload) {
            if let Some(tags) = payload.get("tags").and_then(Value::as_array) {
                for tag in tags.iter().filter_map(Value::as_str) {
                    *counts.entry(tag.to_string()).or_default() += 1;
                }
            }
        }
    }
    let tags: Vec<Value> = counts.into_iter().map(|(name, count)| json!({ "name": name, "count": count })).collect();
    json_response(json!({ "tags": tags }), 200).map_err(AppError::from)
}

async fn rename_tag(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let from = normalize_tag_name(body.get("from").and_then(Value::as_str).unwrap_or(""));
    let to = normalize_tag_name(body.get("to").and_then(Value::as_str).unwrap_or(""));
    if from.is_empty() || to.is_empty() {
        return Err(AppError::new(400, "from and to are required"));
    }
    let rows = if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.content LIKE ?")
            .bind(&[format!("%#{}%", from).into()])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.content LIKE ? AND memo.creator_id = ?")
            .bind(&[format!("%#{}%", from).into(), js_num(viewer.id)])?
            .all()
            .await?
    };
    let memos: Vec<DbMemo> = rows.results()?;
    let mut updated = 0;
    for memo in memos {
        let next_content = replace_tag_in_content(&memo.content, &from, &to);
        if next_content == memo.content {
            continue;
        }
        db(env)?.prepare("UPDATE memo SET content = ?, payload = ?, updated_ts = ? WHERE id = ?")
            .bind(&[next_content.clone().into(), build_memo_payload(&next_content).to_string().into(), js_num(unix_now()), js_num(memo.id)])?
            .run()
            .await?;
        updated += 1;
    }
    json_response(json!({ "updated": updated }), 200).map_err(AppError::from)
}

async fn timeline(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let rows = if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT date(created_ts, 'unixepoch') AS day, COUNT(*) AS count FROM memo WHERE row_status = 'NORMAL' GROUP BY day ORDER BY day DESC LIMIT 120")
            .all().await?
    } else {
        db(env)?.prepare("SELECT date(created_ts, 'unixepoch') AS day, COUNT(*) AS count FROM memo WHERE row_status = 'NORMAL' AND (visibility != 'PRIVATE' OR creator_id = ?) GROUP BY day ORDER BY day DESC LIMIT 120")
            .bind(&[js_num(viewer.id)])?
            .all().await?
    };
    let days: Vec<Value> = rows.results()?;
    json_response(json!({ "days": days }), 200).map_err(AppError::from)
}

async fn get_recent_memos(env: &Env, viewer: &Viewer, limit: i64) -> std::result::Result<Vec<DbMemo>, AppError> {
    let rows = if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id ORDER BY memo.created_ts DESC LIMIT ?")
            .bind(&[js_num(limit)])?
            .all().await?
    } else {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.visibility != 'PRIVATE' OR memo.creator_id = ? ORDER BY memo.created_ts DESC LIMIT ?")
            .bind(&[js_num(viewer.id), js_num(limit)])?
            .all().await?
    };
    Ok(rows.results()?)
}

async fn get_memos_by_uids(env: &Env, viewer: &Viewer, uids: &[String]) -> std::result::Result<Vec<DbMemo>, AppError> {
    let placeholders = placeholders(uids.len());
    let mut values: Vec<JsValue> = uids.iter().map(|uid| uid.clone().into()).collect();
    let sql = if viewer.role == "ADMIN" {
        format!("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.uid IN ({})", placeholders)
    } else {
        values.push(js_num(viewer.id));
        format!("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.uid IN ({}) AND memo.creator_id = ?", placeholders)
    };
    let rows = db(env)?.prepare(sql).bind(&values)?.all().await?;
    Ok(rows.results()?)
}

async fn get_memo_by_uid(env: &Env, uid: &str) -> std::result::Result<Option<DbMemo>, AppError> {
    Ok(db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.uid = ?")
        .bind(&[uid.into()])?
        .first(None)
        .await?)
}

async fn get_user_by_username(env: &Env, username: &str) -> std::result::Result<Option<DbUser>, AppError> {
    Ok(db(env)?.prepare("SELECT * FROM \"user\" WHERE username = ?")
        .bind(&[username.into()])?
        .first(None)
        .await?)
}

async fn get_user_by_id(env: &Env, id: i64) -> std::result::Result<Option<DbUser>, AppError> {
    Ok(db(env)?.prepare("SELECT * FROM \"user\" WHERE id = ?")
        .bind(&[js_num(id)])?
        .first(None)
        .await?)
}

async fn resolve_user(env: &Env, identifier: &str) -> std::result::Result<Option<DbUser>, AppError> {
    let decoded = identifier.trim();
    if let Ok(id) = decoded.parse::<i64>() {
        return get_user_by_id(env, id).await;
    }
    get_user_by_username(env, decoded).await
}

async fn purge_ids(env: &Env, ids: &[i64]) -> std::result::Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = placeholders(ids.len());
    let values: Vec<JsValue> = ids.iter().map(|id| js_num(*id)).collect();
    db(env)?.prepare(format!("UPDATE attachment SET memo_id = NULL, updated_ts = ? WHERE memo_id IN ({})", placeholders))
        .bind(&bind_with_first(unix_now(), ids))?
        .run().await?;
    db(env)?.prepare(format!("DELETE FROM reaction WHERE content_type = 'MEMO' AND content_id IN ({})", placeholders))
        .bind(&values)?.run().await?;
    db(env)?.prepare(format!("DELETE FROM memo_share WHERE memo_id IN ({})", placeholders))
        .bind(&values)?.run().await?;
    let mut relation_values = values.clone();
    relation_values.extend(values.clone());
    db(env)?.prepare(format!("DELETE FROM memo_relation WHERE memo_id IN ({}) OR related_memo_id IN ({})", placeholders, placeholders))
        .bind(&relation_values)?.run().await?;
    db(env)?.prepare(format!("DELETE FROM memo WHERE id IN ({})", placeholders))
        .bind(&values)?.run().await?;
    Ok(())
}

fn public_user(user: DbUser) -> PublicUser {
    PublicUser {
        id: user.id,
        username: user.username,
        role: user.role,
        nickname: user.nickname,
        email: user.email,
        avatar_url: user.avatar_url,
        description: user.description,
    }
}

fn public_memo(memo: DbMemo) -> PublicMemo {
    public_memo_with_attachments(memo, vec![])
}

fn public_memo_with_attachments(memo: DbMemo, attachments: Vec<Value>) -> PublicMemo {
    PublicMemo {
        name: format!("memos/{}", memo.uid),
        id: memo.id,
        uid: memo.uid,
        creator: MemoCreator {
            id: memo.creator_id,
            username: memo.creator_username,
            nickname: memo.creator_nickname,
        },
        created_ts: memo.created_ts,
        updated_ts: memo.updated_ts,
        row_status: memo.row_status,
        content: memo.content,
        visibility: memo.visibility,
        pinned: memo.pinned != 0,
        payload: serde_json::from_str(&memo.payload).unwrap_or_else(|_| json!({})),
        attachments,
    }
}

async fn memo_with_attachments(env: &Env, memo: DbMemo) -> std::result::Result<PublicMemo, AppError> {
    let attachments = list_attachments_for_memo(env, memo.id).await?;
    Ok(public_memo_with_attachments(memo, attachments))
}

async fn list_attachments_for_memo(env: &Env, memo_id: i64) -> std::result::Result<Vec<Value>, AppError> {
    let rows = db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id = ? ORDER BY attachment.created_ts, attachment.id")
        .bind(&[js_num(memo_id)])?
        .all()
        .await?;
    let attachments: Vec<DbAttachment> = rows.results()?;
    Ok(attachments.into_iter().map(public_attachment).collect())
}

fn shared_attachment_url(share_uid: &str, attachment_uid: &str, filename: &str) -> String {
    format!("/api/v1/shares/{}/attachments/{}/{}", share_uid, attachment_uid, filename)
}

fn can_read(memo: &DbMemo, viewer: &Viewer) -> bool {
    viewer.role == "ADMIN" || memo.visibility != "PRIVATE" || memo.creator_id == viewer.id
}

fn can_write(memo: &DbMemo, viewer: &Viewer) -> bool {
    viewer.role == "ADMIN" || memo.creator_id == viewer.id
}

fn require_admin(viewer: &Viewer) -> std::result::Result<(), AppError> {
    if viewer.role == "ADMIN" {
        Ok(())
    } else {
        Err(AppError::new(403, "Forbidden"))
    }
}

fn build_memo_payload(content: &str) -> Value {
    let tags: Vec<String> = content.split_whitespace()
        .filter_map(|word| word.strip_prefix('#'))
        .map(|tag| tag.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '/').to_string())
        .filter(|tag| !tag.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    json!({
        "tags": tags,
        "property": {
            "hasTaskList": content.contains("- [") || content.contains("* ["),
            "hasLink": content.contains("http://") || content.contains("https://"),
            "hasCode": content.contains("```") || content.contains('`'),
            "hasIncompleteTasks": content.contains("[ ]")
        }
    })
}

fn normalize_tag_name(value: &str) -> String {
    value.trim()
        .trim_start_matches('#')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(64)
        .collect()
}

fn replace_tag_in_content(content: &str, from: &str, to: &str) -> String {
    content
        .split_inclusive(char::is_whitespace)
        .map(|token| {
            let trimmed = token.trim_end();
            let suffix = &token[trimmed.len()..];
            if trimmed == format!("#{}", from) {
                format!("#{}{}", to, suffix)
            } else {
                token.to_string()
            }
        })
        .collect()
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn extract_title(content: &str) -> String {
    let first = content.lines().next().unwrap_or("").trim().trim_start_matches('#').trim();
    let cleaned = first.replace(['*', '_', '`', '~'], "");
    if cleaned.is_empty() {
        "Memo".to_string()
    } else {
        let mut title: String = cleaned.chars().take(120).collect();
        if cleaned.chars().count() > 120 {
            title.push_str("...");
        }
        title
    }
}

trait JsValueFallback {
    fn if_undefined(self, fallback: &str) -> JsValue;
}

impl JsValueFallback for JsValue {
    fn if_undefined(self, fallback: &str) -> JsValue {
        if self.is_null() || self.is_undefined() {
            fallback.into()
        } else {
            self
        }
    }
}

fn json_bind(value: Option<&Value>) -> JsValue {
    match value {
        Some(Value::String(text)) => text.clone().into(),
        Some(Value::Number(number)) => number.as_i64().map(js_num).unwrap_or_else(|| JsValue::from_f64(number.as_f64().unwrap_or_default())),
        Some(Value::Bool(value)) => js_num(if *value { 1 } else { 0 }),
        Some(Value::Null) | None => JsValue::NULL,
        Some(other) => other.to_string().into(),
    }
}

fn normalize_http_url(value: impl AsRef<str>, message: &str) -> std::result::Result<String, AppError> {
    let raw = value.as_ref().trim();
    if raw.is_empty() {
        return Err(AppError::new(400, message));
    }
    let mut url = Url::parse(raw).map_err(|_| AppError::new(400, message))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(AppError::new(400, message));
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

async fn record_audit(env: &Env, viewer: Option<&Viewer>, action: &str, target: &str, detail: Value) {
    if ensure_audit_log_table(env).await.is_err() {
        return;
    }
    if let Ok(database) = db(env) {
        let stmt = database.prepare("INSERT INTO audit_log (created_ts, actor_id, action, target, detail) VALUES (?, ?, ?, ?, ?)");
        if let Ok(bound) = stmt.bind(&[
            js_num(unix_now()),
            viewer.map(|user| js_num(user.id)).unwrap_or(JsValue::NULL),
            action.into(),
            target.into(),
            detail.to_string().into(),
        ]) {
            let _ = bound.run().await;
        }
    }
}

async fn ensure_audit_log_table(env: &Env) -> std::result::Result<(), AppError> {
    db(env)?.prepare("CREATE TABLE IF NOT EXISTS audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT, created_ts INTEGER NOT NULL, actor_id INTEGER, action TEXT NOT NULL, target TEXT NOT NULL DEFAULT '', detail TEXT NOT NULL DEFAULT '{}')")
        .run()
        .await?;
    db(env)?.prepare("CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_ts)")
        .run()
        .await?;
    db(env)?.prepare("CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action, created_ts)")
        .run()
        .await?;
    Ok(())
}

fn audit_action_label(action: &str) -> &str {
    match action {
        "memo.delete" => "删除备忘录",
        "memo.purge" => "彻底删除备忘录",
        "attachment.delete" => "删除附件",
        "backup.create" => "创建备份",
        "backup.restore" => "恢复备份",
        "migration.usememos.start" => "开始迁移原版 Memos",
        "migration.usememos.import" => "迁移原版 Memos",
        "migration.usememos.error" => "迁移原版 Memos 失败",
        "webhook.create" => "创建 Webhook",
        "webhook.delete" => "删除 Webhook",
        "tag.rename" => "重命名标签",
        _ => action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_artifact_api_payload_includes_key_and_size() {
        let artifact = BackupArtifact { key: "backups/memos-test.json".to_string(), size: 42 };

        assert_eq!(
            backup_artifact_payload(&artifact),
            json!({ "backup": { "key": "backups/memos-test.json", "size": 42 } })
        );
    }

    #[test]
    fn sanitize_filename_replaces_unsafe_names() {
        assert_eq!(sanitize_filename("../bad:name?.png"), ".._bad_name_.png");
        assert_eq!(sanitize_filename("\u{0000}"), "attachment");
    }

    #[test]
    fn attachment_storage_key_uses_creator_uid_and_filename() {
        assert_eq!(
            attachment_storage_key(7, "a_123", "note.txt"),
            "attachments/7/a_123/note.txt"
        );
    }

    #[test]
    fn public_memo_with_attachments_preserves_attachment_payload() {
        let memo = sample_memo();
        let attachments = vec![json!({ "uid": "a_1", "filename": "note.txt" })];
        let public = public_memo_with_attachments(memo, attachments.clone());

        assert_eq!(public.attachments, attachments);
    }

    #[test]
    fn shared_attachment_url_uses_share_and_attachment_identity() {
        assert_eq!(
            shared_attachment_url("s_1", "a_1", "note.txt"),
            "/api/v1/shares/s_1/attachments/a_1/note.txt"
        );
    }

    #[test]
    fn sse_ready_payload_is_valid_event_stream() {
        let payload = sse_ready_payload(7).expect("ready payload");

        assert!(payload.starts_with("retry: 5000\n"));
        assert!(payload.contains("event: ready\n"));
        assert!(payload.contains("\"userId\":7"));
    }

    #[test]
    fn memo_event_sse_includes_id_event_and_payload() {
        let event = DbMemoEvent {
            id: 42,
            event_type: "memo.updated".to_string(),
            name: "memos/m_1".to_string(),
            visibility: "PRIVATE".to_string(),
            creator_id: 7,
            payload: json!({ "type": "memo.updated", "name": "memos/m_1" }).to_string(),
        };
        let payload = memo_event_sse(&event).expect("memo event");

        assert!(payload.starts_with("id: 42\nevent: memo.updated\n"));
        assert!(payload.contains("\"id\":\"42\""));
        assert!(payload.contains("\"creatorId\":7"));
    }

    #[test]
    fn sse_since_id_prefers_last_event_id() {
        let url = Url::parse("https://memos.local/api/v1/sse?since=7").expect("url");

        assert_eq!(sse_since_id(Some("42"), &url), Some(42));
        assert_eq!(sse_since_id(None, &url), Some(7));
        assert_eq!(sse_since_id(Some("bad"), &url), Some(7));
    }

    #[test]
    fn memo_event_payload_matches_frontend_refresh_shape() {
        let memo = sample_memo();

        assert_eq!(
            memo_event_payload("memo.updated", &memo),
            json!({
                "type": "memo.updated",
                "name": "memos/m_1",
                "visibility": "PRIVATE",
                "creatorId": 7
            })
        );
    }

    #[test]
    fn memo_event_payload_merges_detail_fields() {
        let memo = sample_memo();

        assert_eq!(
            memo_event_payload_with_detail(
                "memo.bulk.updated",
                &memo,
                json!({ "action": "ARCHIVE", "updated": 2, "deleted": 0 })
            ),
            json!({
                "type": "memo.bulk.updated",
                "name": "memos/m_1",
                "visibility": "PRIVATE",
                "creatorId": 7,
                "action": "ARCHIVE",
                "updated": 2,
                "deleted": 0
            })
        );
    }

    #[test]
    fn memo_webhook_body_wraps_event_timestamp_and_public_memo() {
        let memo = sample_memo();

        assert_eq!(
            memo_webhook_body("memo.updated", &memo, 1779345600, json!({ "source": "test" })),
            json!({
                "event": "memo.updated",
                "timestamp": 1779345600,
                "payload": {
                    "memo": {
                        "name": "memos/m_1",
                        "id": 1,
                        "uid": "m_1",
                        "creator": { "id": 7, "username": "alice", "nickname": "Alice" },
                        "createdTs": 10,
                        "updatedTs": 11,
                        "rowStatus": "NORMAL",
                        "content": "hello",
                        "visibility": "PRIVATE",
                        "pinned": false,
                        "payload": {},
                        "attachments": []
                    },
                    "detail": { "source": "test" }
                }
            })
        );
    }

    #[test]
    fn comment_inbox_message_points_to_parent_and_comment() {
        assert_eq!(
            comment_inbox_message("m_parent", "m_comment"),
            json!({
                "type": "memo.comment.created",
                "memoUid": "m_parent",
                "commentUid": "m_comment"
            })
        );
    }

    #[test]
    fn memo_event_retention_cutoff_uses_whole_days() {
        assert_eq!(memo_event_retention_cutoff(1_000_000, 7), 395_200);
        assert_eq!(memo_event_retention_cutoff(1_000_000, -1), 1_000_000);
    }

    #[test]
    fn parse_user_settings_path_splits_identifier_and_key() {
        assert_eq!(
            parse_user_settings_path("alice/settings/theme"),
            Some(("alice", Some("theme")))
        );
        assert_eq!(
            parse_user_settings_path("alice/settings"),
            Some(("alice", None))
        );
        assert_eq!(parse_user_settings_path("alice"), None);
    }

    #[test]
    fn safe_inbox_message_parse_falls_back_to_unknown() {
        assert_eq!(safe_inbox_message("{\"type\":\"memo.comment.created\",\"memoUid\":\"m_1\"}")["type"], "memo.comment.created");
        assert_eq!(safe_inbox_message("not json"), json!({ "type": "unknown" }));
    }

    #[test]
    fn memo_child_routes_classify_unknown_paths_as_unsupported() {
        assert_eq!(
            memo_child_route(&["m_1", "relations", "suggest"], &Method::Post),
            MemoChildRoute::SuggestRelations
        );
        assert_eq!(
            memo_child_route(&["m_1", "relations", "suggest"], &Method::Get),
            MemoChildRoute::Unsupported
        );
        assert_eq!(
            memo_child_route(&["m_1", "unknown"], &Method::Get),
            MemoChildRoute::Unsupported
        );
    }

    #[test]
    fn relation_candidates_rank_shared_tags_and_keywords_first() {
        let current = RelationCandidate {
            uid: "m_current".to_string(),
            content: "今天研究 Memos 迁移，想把导入的数据做成知识图谱".to_string(),
            payload: json!({ "tags": ["memos", "graph"] }).to_string(),
            updated_ts: 2000,
        };
        let mut candidates = vec![RelationCandidate {
            uid: "m_related".to_string(),
            content: "Memos 导入之后可以通过引用关系形成 graph".to_string(),
            payload: json!({ "tags": ["memos"] }).to_string(),
            updated_ts: 1000,
        }];
        candidates.extend((0..60).map(|index| RelationCandidate {
            uid: format!("m_{}", index),
            content: format!("普通日记 {}", index),
            payload: "{}".to_string(),
            updated_ts: 900 - index,
        }));

        let ranked = rank_relation_candidates(&current, &candidates, 30);

        assert_eq!(ranked.len(), 30);
        assert_eq!(ranked[0].uid, "m_related");
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn parse_ai_relation_suggestions_drops_unknown_memos() {
        let mut candidates = HashMap::new();
        candidates.insert("m_related".to_string(), "content".to_string());
        let parsed = parse_ai_relation_suggestions(
            &json!({
                "suggestions": [
                    { "memo": "memos/m_related", "reason": "都在讨论 Memos 迁移", "confidence": 0.82 },
                    { "memo": "memos/m_missing", "reason": "不存在", "confidence": 0.9 }
                ]
            }).to_string(),
            &candidates,
        );

        assert_eq!(parsed, vec![json!({
            "memo": "memos/m_related",
            "content": "content",
            "reason": "都在讨论 Memos 迁移",
            "confidence": 0.82,
            "source": "ai"
        })]);
    }

    fn sample_memo() -> DbMemo {
        DbMemo {
            id: 1,
            uid: "m_1".to_string(),
            creator_id: 7,
            creator_username: "alice".to_string(),
            creator_nickname: "Alice".to_string(),
            created_ts: 10,
            updated_ts: 11,
            row_status: "NORMAL".to_string(),
            content: "hello".to_string(),
            visibility: "PRIVATE".to_string(),
            pinned: 0,
            payload: "{}".to_string(),
        }
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut text: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        text.push_str("...");
    }
    text
}

async fn fetch_asset(req: &Request, env: &Env) -> std::result::Result<Response, AppError> {
    let assets = env.assets("ASSETS")?;
    let response = assets.fetch_request(req.clone()?).await?;
    if response.status_code() != 404 {
        return Ok(response);
    }
    let index_req = Request::new_with_init("/index.html", &RequestInit::new())?;
    Ok(assets.fetch_request(index_req).await?)
}

fn db(env: &Env) -> std::result::Result<D1Database, AppError> {
    Ok(env.d1("DB")?)
}

fn server_secret(env: &Env) -> std::result::Result<String, AppError> {
    if let Ok(secret) = env.secret("SERVER_SECRET") {
        return Ok(secret.to_string());
    }
    env.var("SERVER_SECRET").map(|value| value.to_string()).map_err(|_| AppError::new(500, "SERVER_SECRET is not configured"))
}

fn normalize_username(value: Option<&str>) -> std::result::Result<String, AppError> {
    let username = value.unwrap_or("").trim().to_lowercase();
    let valid = username.len() >= 3
        && username.len() <= 32
        && username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        && username.chars().next().map(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_').unwrap_or(false);
    if valid {
        Ok(username)
    } else {
        Err(AppError::new(400, "Username must be 3-32 lowercase letters, numbers, _ or -"))
    }
}

fn assert_password(value: &str) -> std::result::Result<(), AppError> {
    if value.len() >= 8 {
        Ok(())
    } else {
        Err(AppError::new(400, "Password must be at least 8 characters"))
    }
}

fn normalize_visibility(value: &str) -> std::result::Result<String, AppError> {
    let visibility = value.to_uppercase();
    if matches!(visibility.as_str(), "PUBLIC" | "PROTECTED" | "PRIVATE") {
        Ok(visibility)
    } else {
        Err(AppError::new(400, "Invalid visibility"))
    }
}

fn normalize_state(value: &str) -> std::result::Result<String, AppError> {
    let state = value.to_uppercase();
    if matches!(state.as_str(), "NORMAL" | "ARCHIVED") {
        Ok(state)
    } else {
        Err(AppError::new(400, "Invalid row status"))
    }
}

fn hash_password(password: &str) -> String {
    let salt = random_bytes(16);
    let mut derived = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, 100_000, &mut derived);
    format!("pbkdf2_sha256$100000${}${}", base64url(&salt), base64url(&derived))
}

fn verify_password(password: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2_sha256" {
        return false;
    }
    let iterations = parts[1].parse::<u32>().unwrap_or(0);
    if iterations == 0 {
        return false;
    }
    let salt = match URL_SAFE_NO_PAD.decode(parts[2]) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let expected = match URL_SAFE_NO_PAD.decode(parts[3]) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let mut actual = vec![0u8; expected.len()];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iterations, &mut actual);
    constant_time_equal(&actual, &expected)
}

fn sign_jwt(claims: &Claims, secret: &str) -> std::result::Result<String, AppError> {
    let header = base64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = base64url(serde_json::to_string(claims).map_err(|_| AppError::new(500, "JWT encode failed"))?.as_bytes());
    let data = format!("{}.{}", header, payload);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| AppError::new(500, "JWT key failed"))?;
    mac.update(data.as_bytes());
    let signature = mac.finalize().into_bytes();
    Ok(format!("{}.{}", data, base64url(&signature)))
}

fn verify_jwt(token: &str, secret: &str) -> std::result::Result<Claims, AppError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(AppError::new(401, "Invalid token"));
    }
    let data = format!("{}.{}", parts[0], parts[1]);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| AppError::new(500, "JWT key failed"))?;
    mac.update(data.as_bytes());
    let expected = mac.finalize().into_bytes();
    let actual = URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| AppError::new(401, "Invalid token"))?;
    if !constant_time_equal(&actual, &expected) {
        return Err(AppError::new(401, "Invalid token"));
    }
    let payload = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| AppError::new(401, "Invalid token"))?;
    let claims: Claims = serde_json::from_slice(&payload).map_err(|_| AppError::new(401, "Invalid token"))?;
    if claims.iss != "memos-worker" || claims.exp < unix_now() {
        return Err(AppError::new(401, "Invalid token"));
    }
    Ok(claims)
}

fn base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    for byte in &mut bytes {
        *byte = (js_sys::Math::random() * 256.0).floor() as u8;
    }
    bytes
}

fn generate_uid(prefix: &str) -> String {
    format!("{}_{}", prefix, base64url(&random_bytes(12)))
}

fn unix_now() -> i64 {
    js_sys::Date::now() as i64 / 1000
}

fn parse_cookies(header: &str) -> HashMap<String, String> {
    header.split(';')
        .filter_map(|part| {
            let mut chunks = part.trim().splitn(2, '=');
            Some((chunks.next()?.to_string(), chunks.next().unwrap_or("").to_string()))
        })
        .collect()
}

fn cookie(name: &str, value: &str, max_age: i64, http_only: bool) -> String {
    let mut parts = vec![
        format!("{}={}", name, value),
        "Path=/api/v1".to_string(),
        format!("Max-Age={}", max_age),
        "SameSite=Lax".to_string(),
        "Secure".to_string(),
    ];
    if http_only {
        parts.push("HttpOnly".to_string());
    }
    parts.join("; ")
}

fn clear_cookie(name: &str) -> String {
    format!("{}=; Path=/api/v1; Max-Age=0; SameSite=Lax; Secure; HttpOnly", name)
}

fn append_cookie(response: &mut Response, value: &str) {
    let _ = response.headers_mut().append("Set-Cookie", value);
}

fn json_response(body: Value, status: u16) -> Result<Response> {
    let mut response = Response::from_json(&body)?.with_status(status);
    response.headers_mut().set("Access-Control-Allow-Origin", "*")?;
    response.headers_mut().set("Access-Control-Allow-Methods", "GET,POST,PATCH,DELETE,OPTIONS")?;
    response.headers_mut().set("Access-Control-Allow-Headers", "Content-Type,Authorization")?;
    Ok(response)
}

fn empty_response(status: u16) -> Response {
    let mut response = Response::empty().unwrap().with_status(status);
    let _ = response.headers_mut().set("Access-Control-Allow-Origin", "*");
    let _ = response.headers_mut().set("Access-Control-Allow-Methods", "GET,POST,PATCH,DELETE,OPTIONS");
    let _ = response.headers_mut().set("Access-Control-Allow-Headers", "Content-Type,Authorization");
    response
}

fn extract_filter_value(url: &Url, field: &str) -> Option<String> {
    let filter = url.query_pairs().find(|(key, _)| key == "filter")?.1.to_string();
    let needle = format!("{} == \"", field);
    let start = filter.find(&needle)? + needle.len();
    let rest = &filter[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn query_param(url: &Url, field: &str) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key == field)
        .map(|(_, value)| value.to_string())
}

fn extract_content_contains_filter(url: &Url) -> Option<String> {
    let filter = query_param(url, "filter")?;
    let needle = "content.contains(\"";
    let start = filter.find(needle)? + needle.len();
    let rest = &filter[start..];
    let mut value = String::new();
    let mut escaped = false;
    for ch in rest.chars() {
        if escaped {
            value.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(value);
        } else {
            value.push(ch);
        }
    }
    None
}

fn escape_like(value: &str) -> String {
    value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn placeholders(count: usize) -> String {
    std::iter::repeat("?").take(count).collect::<Vec<_>>().join(",")
}

fn bind_with_first(first: i64, ids: &[i64]) -> Vec<JsValue> {
    let mut values = vec![js_num(first)];
    values.extend(ids.iter().map(|id| js_num(*id)));
    values
}

fn js_num(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}
