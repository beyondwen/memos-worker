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

include!("models.rs");

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
        let username = path
            .trim_start_matches("/api/v1/u/")
            .trim_end_matches("/rss.xml")
            .trim_end_matches('/');
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
        let user = get_user_by_id(env, viewer.id)
            .await?
            .ok_or_else(|| AppError::new(401, "User unavailable"))?;
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
        let identifier = path
            .trim_start_matches("/api/v1/users/")
            .trim_end_matches("/stats")
            .trim_end_matches('/');
        return user_stats(env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/")
        && path.ends_with("/access-tokens")
        && method == Method::Get
    {
        let identifier = path
            .trim_start_matches("/api/v1/users/")
            .trim_end_matches("/access-tokens")
            .trim_end_matches('/');
        return list_access_tokens(env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/")
        && path.ends_with("/access-tokens")
        && method == Method::Post
    {
        let identifier = path
            .trim_start_matches("/api/v1/users/")
            .trim_end_matches("/access-tokens")
            .trim_end_matches('/');
        return create_access_token(req, env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/")
        && path.contains("/access-tokens/")
        && method == Method::Delete
    {
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
    if path.starts_with("/api/v1/webhooks/deliveries/")
        && path.ends_with("/retry")
        && method == Method::Post
    {
        let id = path
            .trim_start_matches("/api/v1/webhooks/deliveries/")
            .trim_end_matches("/retry")
            .trim_matches('/');
        return retry_webhook_delivery(env, &viewer, id).await;
    }
    if path.starts_with("/api/v1/webhooks/") && path.ends_with("/test") && method == Method::Post {
        let id = path
            .trim_start_matches("/api/v1/webhooks/")
            .trim_end_matches("/test")
            .trim_matches('/');
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
    let count: Option<i64> = db
        .prepare("SELECT COUNT(*) AS count FROM \"user\"")
        .first(Some("count"))
        .await?;
    json_response(
        json!({ "name": "Memos Worker", "setupRequired": count.unwrap_or(0) == 0 }),
        200,
    )
    .map_err(AppError::from)
}

async fn setup_admin(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let db = db(env)?;
    let count: Option<i64> = db
        .prepare("SELECT COUNT(*) AS count FROM \"user\"")
        .first(Some("count"))
        .await?;
    if count.unwrap_or(0) > 0 {
        return Err(AppError::new(409, "Instance already initialized"));
    }
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let username = normalize_username(body.get("username").and_then(Value::as_str))?;
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Password is required"))?;
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
    let user = get_user_by_username(env, &username)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create admin"))?;
    create_auth_response(env, req, user, 201).await
}

async fn sign_up(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let username = normalize_username(body.get("username").and_then(Value::as_str))?;
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Password is required"))?;
    assert_password(password)?;
    if get_user_by_username(env, &username).await?.is_some() {
        return Err(AppError::new(409, "Username already exists"));
    }
    let user_count: Option<i64> = db(env)?
        .prepare("SELECT COUNT(*) AS count FROM \"user\"")
        .first(Some("count"))
        .await?;
    let role = if user_count.unwrap_or(0) == 0 {
        "ADMIN"
    } else {
        "USER"
    };
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
    let user = get_user_by_username(env, &username)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create user"))?;
    create_auth_response(env, req, user, 201).await
}

async fn sign_in(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let username = normalize_username(body.get("username").and_then(Value::as_str))?;
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Password is required"))?;
    let user = get_user_by_username(env, &username)
        .await?
        .ok_or_else(|| AppError::new(401, "Invalid username or password"))?;
    if user.row_status != "NORMAL" || !verify_password(password, &user.password_hash) {
        return Err(AppError::new(401, "Invalid username or password"));
    }
    create_auth_response(env, req, user, 200).await
}

async fn refresh_session(req: &Request, env: &Env) -> std::result::Result<Response, AppError> {
    let cookies = parse_cookies(
        req.headers()
            .get("Cookie")
            .ok()
            .flatten()
            .unwrap_or_default()
            .as_str(),
    );
    let refresh = cookies
        .get(REFRESH_COOKIE)
        .ok_or_else(|| AppError::new(401, "Missing refresh token"))?;
    let claims = verify_jwt(refresh, &server_secret(env)?)?;
    if claims.token_type != "refresh" {
        return Err(AppError::new(401, "Invalid refresh token"));
    }
    let user_id = claims
        .sub
        .parse::<i64>()
        .map_err(|_| AppError::new(401, "Invalid token subject"))?;
    let user = get_user_by_id(env, user_id)
        .await?
        .ok_or_else(|| AppError::new(401, "User unavailable"))?;
    create_auth_response(env, req, user, 200).await
}

fn sign_out() -> std::result::Result<Response, AppError> {
    let mut res = json_response(json!({ "ok": true }), 200).map_err(AppError::from)?;
    append_cookie(&mut res, &clear_cookie(REFRESH_COOKIE));
    append_cookie(&mut res, &clear_cookie(ACCESS_COOKIE));
    Ok(res)
}

async fn create_auth_response(
    env: &Env,
    req: &Request,
    user: DbUser,
    status: u16,
) -> std::result::Result<Response, AppError> {
    let now = unix_now();
    let session_id = generate_uid("s");
    let refresh_token = sign_jwt(
        &Claims {
            iss: "memos-worker".to_string(),
            sub: user.id.to_string(),
            exp: now + REFRESH_TTL,
            iat: now,
            token_type: "refresh".to_string(),
            tid: Some(session_id.clone()),
        },
        &server_secret(env)?,
    )?;
    let access_token = sign_jwt(
        &Claims {
            iss: "memos-worker".to_string(),
            sub: user.id.to_string(),
            exp: now + ACCESS_TTL,
            iat: now,
            token_type: "access".to_string(),
            tid: Some(session_id.clone()),
        },
        &server_secret(env)?,
    )?;
    let token_hash = sha256_hex(&refresh_token);
    let ua = req
        .headers()
        .get("User-Agent")
        .ok()
        .flatten()
        .unwrap_or_default();
    db(env)?.prepare("INSERT OR REPLACE INTO user_session (id, user_id, refresh_token_hash, created_ts, updated_ts, expires_ts, user_agent, ip_address) VALUES (?, ?, ?, ?, ?, ?, ?, '')")
        .bind(&[session_id.into(), js_num(user.id), token_hash.into(), js_num(now), js_num(now), js_num(now + REFRESH_TTL), ua.into()])?
        .run()
        .await?;
    let mut res = json_response(
        json!({ "accessToken": access_token, "user": public_user(user) }),
        status,
    )
    .map_err(AppError::from)?;
    append_cookie(
        &mut res,
        &cookie(REFRESH_COOKIE, &refresh_token, REFRESH_TTL, true),
    );
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
        let cookies = parse_cookies(
            req.headers()
                .get("Cookie")
                .ok()
                .flatten()
                .unwrap_or_default()
                .as_str(),
        );
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
    let id = claims
        .sub
        .parse::<i64>()
        .map_err(|_| AppError::new(401, "Unauthorized"))?;
    let user = get_user_by_id(env, id)
        .await?
        .ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    Ok(Viewer {
        id: user.id,
        role: user.role,
    })
}

async fn viewer_from_access_token(env: &Env, token: &str) -> std::result::Result<Viewer, AppError> {
    let prefix: String = token.chars().take(20).collect();
    let token_hash = sha256_hex(token);
    let row: Option<Value> = db(env)?.prepare("SELECT user_id FROM user_access_token WHERE token_prefix = ? AND token_hash = ? AND row_status = 'NORMAL' AND (expires_ts IS NULL OR expires_ts > ?)")
        .bind(&[prefix.into(), token_hash.clone().into(), js_num(unix_now())])?
        .first(None)
        .await?;
    let user_id = row
        .and_then(|value| {
            value
                .get("user_id")
                .and_then(Value::as_i64)
                .or_else(|| value.get("USER_ID").and_then(Value::as_i64))
                .map(|id| id)
        })
        .ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    db(env)?
        .prepare("UPDATE user_access_token SET last_used_ts = ? WHERE token_hash = ?")
        .bind(&[js_num(unix_now()), token_hash.into()])?
        .run()
        .await?;
    let user = get_user_by_id(env, user_id)
        .await?
        .ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    Ok(Viewer {
        id: user.id,
        role: user.role,
    })
}

async fn list_memos(
    env: &Env,
    url: &Url,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let db = db(env)?;
    let limit = url
        .query_pairs()
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
    if let Some(visibility) =
        query_param(url, "visibility").filter(|value| !value.trim().is_empty())
    {
        where_sql.push("memo.visibility = ?".to_string());
        values.push(normalize_visibility(&visibility)?.into());
    }
    if let Some(tag) = query_param(url, "tag").filter(|value| !value.trim().is_empty()) {
        where_sql.push(
            "EXISTS (SELECT 1 FROM json_each(memo.payload, '$.tags') WHERE value = ?)".to_string(),
        );
        values.push(tag.into());
    }
    if let Some(search) =
        extract_content_contains_filter(url).filter(|value| !value.trim().is_empty())
    {
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
    let next_page_token = if has_more {
        (offset + limit).to_string()
    } else {
        String::new()
    };
    json_response(
        json!({ "memos": public, "nextPageToken": next_page_token }),
        200,
    )
    .map_err(AppError::from)
}

async fn create_memo(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if content.is_empty() {
        return Err(AppError::new(400, "Content is required"));
    }
    let visibility = normalize_visibility(
        body.get("visibility")
            .and_then(Value::as_str)
            .unwrap_or("PRIVATE"),
    )?;
    let uid = generate_uid("m");
    let now = unix_now();
    let payload = build_memo_payload(&content);
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, payload) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&[uid.clone().into(), js_num(viewer.id), js_num(now), js_num(now), content.into(), visibility.into(), payload.to_string().into()])?
        .run()
        .await?;
    let memo = get_memo_by_uid(env, &uid)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create memo"))?;
    emit_memo_event(env, "memo.created", &memo).await;
    let memo = memo_with_attachments(env, memo).await?;
    json_response(json!({ "memo": memo }), 201).map_err(AppError::from)
}

async fn memo_subroute(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    raw: &str,
    method: Method,
    url: &Url,
) -> std::result::Result<Response, AppError> {
    let parts: Vec<&str> = raw.split('/').collect();
    let uid = parts[0];
    if parts.len() > 1 {
        match memo_child_route(&parts, &method) {
            MemoChildRoute::ListComments => return list_comments(env, viewer, uid).await,
            MemoChildRoute::CreateComment => return create_comment(req, env, viewer, uid).await,
            MemoChildRoute::ListReactions => return list_reactions(env, viewer, uid).await,
            MemoChildRoute::UpsertReaction => return upsert_reaction(req, env, viewer, uid).await,
            MemoChildRoute::DeleteReaction(reaction_id) => {
                return delete_reaction(env, viewer, uid, reaction_id).await
            }
            MemoChildRoute::GetRelations => return get_relations(env, viewer, uid).await,
            MemoChildRoute::SuggestRelations => {
                return suggest_memo_relations(env, viewer, uid).await
            }
            MemoChildRoute::SetRelations => return set_relations(req, env, viewer, uid).await,
            MemoChildRoute::ListShares => return list_shares(env, viewer, uid).await,
            MemoChildRoute::CreateShare => return create_share(req, env, viewer, uid).await,
            MemoChildRoute::DeleteShare(share_id) => {
                return delete_share(env, viewer, uid, share_id).await
            }
            MemoChildRoute::Unsupported => {
                return Err(AppError::new(404, "Memo subroute not found"))
            }
        }
    }
    match method {
        Method::Get => {
            let memo = get_memo_by_uid(env, uid)
                .await?
                .ok_or_else(|| AppError::new(404, "Memo not found"))?;
            if !can_read(&memo, viewer) {
                return Err(AppError::new(403, "Forbidden"));
            }
            let memo = memo_with_attachments(env, memo).await?;
            json_response(json!({ "memo": memo }), 200).map_err(AppError::from)
        }
        Method::Patch => update_memo(req, env, viewer, uid).await,
        Method::Delete => {
            let memo = get_memo_by_uid(env, uid)
                .await?
                .ok_or_else(|| AppError::new(404, "Memo not found"))?;
            if !can_write(&memo, viewer) {
                return Err(AppError::new(403, "Forbidden"));
            }
            if url.query_pairs().any(|(k, v)| k == "purge" && v == "true") {
                purge_ids(env, &[memo.id]).await?;
                emit_memo_event(env, "memo.deleted", &memo).await;
            } else {
                db(env)?
                    .prepare("UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id = ?")
                    .bind(&[js_num(unix_now()), js_num(memo.id)])?
                    .run()
                    .await?;
                let archived = DbMemo {
                    row_status: "ARCHIVED".to_string(),
                    updated_ts: unix_now(),
                    ..memo.clone()
                };
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
        (Some("reactions"), Method::Delete) if parts.len() > 2 => {
            MemoChildRoute::DeleteReaction(parts[2])
        }
        (Some("relations"), Method::Get) if parts.len() == 2 => MemoChildRoute::GetRelations,
        (Some("relations"), Method::Post) if parts.get(2) == Some(&"suggest") => {
            MemoChildRoute::SuggestRelations
        }
        (Some("relations"), Method::Patch) if parts.len() == 2 => MemoChildRoute::SetRelations,
        (Some("shares"), Method::Get) => MemoChildRoute::ListShares,
        (Some("shares"), Method::Post) => MemoChildRoute::CreateShare,
        (Some("shares"), Method::Delete) if parts.len() > 2 => {
            MemoChildRoute::DeleteShare(parts[2])
        }
        _ => MemoChildRoute::Unsupported,
    }
}

async fn update_memo(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    uid: &str,
) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_write(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or(&memo.content)
        .trim()
        .to_string();
    let visibility = normalize_visibility(
        body.get("visibility")
            .and_then(Value::as_str)
            .unwrap_or(&memo.visibility),
    )?;
    let row_status = normalize_state(
        body.get("rowStatus")
            .or_else(|| body.get("row_status"))
            .and_then(Value::as_str)
            .unwrap_or(&memo.row_status),
    )?;
    let pinned = body
        .get("pinned")
        .and_then(Value::as_bool)
        .map(|value| if value { 1 } else { 0 })
        .unwrap_or(memo.pinned);
    let payload = build_memo_payload(&content);
    db(env)?.prepare("UPDATE memo SET updated_ts = ?, content = ?, visibility = ?, pinned = ?, row_status = ?, payload = ? WHERE id = ?")
        .bind(&[js_num(unix_now()), content.into(), visibility.into(), js_num(pinned), row_status.into(), payload.to_string().into(), js_num(memo.id)])?
        .run()
        .await?;
    let updated = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(500, "Memo disappeared"))?;
    let event_type = if memo.row_status != updated.row_status {
        if updated.row_status == "ARCHIVED" {
            "memo.archived"
        } else {
            "memo.restored"
        }
    } else {
        "memo.updated"
    };
    emit_memo_event(env, event_type, &updated).await;
    let updated = memo_with_attachments(env, updated).await?;
    json_response(json!({ "memo": updated }), 200).map_err(AppError::from)
}

async fn bulk_memos(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let action = body
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_uppercase();
    let mut seen = BTreeSet::new();
    let uids: Vec<String> = body
        .get("memoUids")
        .and_then(Value::as_array)
        .unwrap_or(&Vec::new())
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
    let result =
        json!({ "updated": 0, "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) });
    if ids.is_empty() {
        return json_response(result, 200).map_err(AppError::from);
    }
    let placeholders = placeholders(ids.len());
    let now = unix_now();
    match action.as_str() {
        "ARCHIVE" => {
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&bind_with_first(now, &ids))?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "ARCHIVE",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                Some("ARCHIVED"),
                None,
            )
            .await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "RESTORE" => {
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET row_status = 'NORMAL', updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&bind_with_first(now, &ids))?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "RESTORE",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                Some("NORMAL"),
                None,
            )
            .await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "DELETE" => {
            purge_ids(env, &ids).await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "DELETE",
                0,
                ids.len(),
                uids.len().saturating_sub(ids.len()),
                now,
                None,
                None,
            )
            .await;
            json_response(json!({ "updated": 0, "deleted": ids.len(), "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "VISIBILITY" => {
            let visibility =
                normalize_visibility(body.get("visibility").and_then(Value::as_str).unwrap_or(""))?;
            let mut values = vec![visibility.clone().into(), js_num(now)];
            values.extend(ids.iter().map(|id| js_num(*id)));
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET visibility = ?, updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&values)?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "VISIBILITY",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                None,
                Some(&visibility),
            )
            .await;
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
    let memos = db(env)?
        .prepare("SELECT * FROM memo ORDER BY created_ts, id")
        .all()
        .await?;
    let attachments = db(env)?.prepare("SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload FROM attachment ORDER BY created_ts, id")
        .all()
        .await?;
    let users: Vec<Value> = users.results()?;
    let memos: Vec<Value> = memos.results()?;
    let attachments: Vec<Value> = attachments.results()?;
    json_response(
        json!({
            "exportedAt": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default(),
            "users": users,
            "memos": memos,
            "attachments": attachments
        }),
        200,
    )
    .map_err(AppError::from)
}

async fn import_data(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let mut imported = 0;
    let now = unix_now();
    if let Some(memos) = body.get("memos").and_then(Value::as_array) {
        for item in memos {
            let content = item
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if content.is_empty() {
                continue;
            }
            let uid = item
                .get("uid")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| generate_uid("m"));
            let created_ts = item
                .get("created_ts")
                .or_else(|| item.get("createdTs"))
                .and_then(Value::as_i64)
                .unwrap_or(now);
            let updated_ts = item
                .get("updated_ts")
                .or_else(|| item.get("updatedTs"))
                .and_then(Value::as_i64)
                .unwrap_or(created_ts);
            let row_status = normalize_state(
                item.get("row_status")
                    .or_else(|| item.get("rowStatus"))
                    .and_then(Value::as_str)
                    .unwrap_or("NORMAL"),
            )?;
            let visibility = normalize_visibility(
                item.get("visibility")
                    .and_then(Value::as_str)
                    .unwrap_or("PRIVATE"),
            )?;
            let pinned = item
                .get("pinned")
                .and_then(Value::as_bool)
                .map(|value| if value { 1 } else { 0 })
                .or_else(|| item.get("pinned").and_then(Value::as_i64))
                .unwrap_or(0);
            let payload = item
                .get("payload")
                .cloned()
                .unwrap_or_else(|| build_memo_payload(content));
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
    memo.attachments = memo
        .attachments
        .into_iter()
        .map(|attachment| {
            let attachment_uid = attachment
                .get("uid")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let filename = attachment
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or("attachment")
                .to_string();
            let mut next = attachment;
            if let Some(object) = next.as_object_mut() {
                object.insert(
                    "url".to_string(),
                    shared_attachment_url(uid, &attachment_uid, &filename).into(),
                );
            }
            next
        })
        .collect();
    json_response(json!({ "memo": memo }), 200).map_err(AppError::from)
}

async fn download_shared_attachment(
    env: &Env,
    share_uid: &str,
    attachment_uid: &str,
) -> std::result::Result<Response, AppError> {
    let attachment: Option<DbAttachment> = db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment JOIN memo_share ON memo_share.memo_id = attachment.memo_id JOIN memo ON memo.id = memo_share.memo_id WHERE memo_share.uid = ? AND attachment.uid = ? AND memo.row_status = 'NORMAL' AND (memo_share.expires_ts IS NULL OR memo_share.expires_ts > ?)")
        .bind(&[share_uid.into(), attachment_uid.into(), js_num(unix_now())])?
        .first(None)
        .await?;
    let attachment = attachment.ok_or_else(|| AppError::new(404, "Attachment not found"))?;
    let object = env
        .bucket("MEMOS_BUCKET")?
        .get(attachment.reference.clone())
        .execute()
        .await?
        .ok_or_else(|| AppError::new(404, "File not found"))?;
    let body = object
        .body()
        .ok_or_else(|| AppError::new(404, "File not found"))?
        .response_body()?;
    let mut response = ResponseBuilder::new().body(body);
    response.headers_mut().set(
        "Content-Type",
        if attachment.file_type.is_empty() {
            "application/octet-stream"
        } else {
            &attachment.file_type
        },
    )?;
    response.headers_mut().set(
        "Content-Disposition",
        &format!("inline; filename=\"{}\"", attachment.filename),
    )?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=3600")?;
    Ok(response)
}

async fn generate_rss(
    env: &Env,
    username: Option<&str>,
) -> std::result::Result<Response, AppError> {
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
    let self_path = username
        .map(|name| format!("/api/v1/u/{}/rss.xml", name))
        .unwrap_or_else(|| "/api/v1/explore/rss.xml".to_string());
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n  <channel>\n    <title>Memos Worker</title>\n    <link>/</link>\n    <description>A lightweight memo hub</description>\n    <lastBuildDate>{}</lastBuildDate>\n    <atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\"/>\n{}\n  </channel>\n</rss>",
        js_sys::Date::new_0().to_utc_string().as_string().unwrap_or_default(),
        escape_xml(&self_path),
        items
    );
    let mut response = Response::ok(xml)?;
    response
        .headers_mut()
        .set("Content-Type", "application/rss+xml; charset=utf-8")?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=600")?;
    Ok(response)
}

include!("migration.rs");

include!("integrations.rs");

include!("social.rs");

async fn update_me(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
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
    let user = get_user_by_id(env, viewer.id)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    json_response(json!({ "user": public_user(user) }), 200).map_err(AppError::from)
}

async fn change_password(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let current = body
        .get("currentPassword")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Current password is required"))?;
    let new_password = body
        .get("newPassword")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Password is required"))?;
    assert_password(new_password)?;
    let user = get_user_by_id(env, viewer.id)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if !verify_password(current, &user.password_hash) {
        return Err(AppError::new(401, "Current password is incorrect"));
    }
    db(env)?
        .prepare("UPDATE \"user\" SET password_hash = ?, updated_ts = ? WHERE id = ?")
        .bind(&[
            hash_password(new_password).into(),
            js_num(unix_now()),
            js_num(viewer.id),
        ])?
        .run()
        .await?;
    sign_out()
}

async fn list_users(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let rows = db(env)?
        .prepare("SELECT * FROM \"user\" ORDER BY id")
        .all()
        .await?;
    let users: Vec<DbUser> = rows.results()?;
    let payload: Vec<PublicUser> = users.into_iter().map(public_user).collect();
    json_response(json!({ "users": payload }), 200).map_err(AppError::from)
}

async fn user_subroute(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
    method: Method,
) -> std::result::Result<Response, AppError> {
    if let Some((user_identifier, key)) = parse_user_settings_path(identifier) {
        return match (key, method) {
            (None, Method::Get) => list_user_settings(env, viewer, user_identifier).await,
            (Some(setting_key), Method::Get) => {
                get_user_setting(env, viewer, user_identifier, setting_key).await
            }
            (Some(setting_key), Method::Patch) => {
                update_user_setting(req, env, viewer, user_identifier, setting_key).await
            }
            _ => Err(AppError::new(405, "Method not allowed")),
        };
    }

    match method {
        Method::Get => {
            let user = resolve_user(env, identifier)
                .await?
                .ok_or_else(|| AppError::new(404, "User not found"))?;
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

async fn update_user(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let role = if viewer.role == "ADMIN" {
        body.get("role")
            .and_then(Value::as_str)
            .filter(|role| matches!(*role, "ADMIN" | "USER"))
            .unwrap_or(&user.role)
    } else {
        &user.role
    };
    let row_status = body
        .get("rowStatus")
        .and_then(Value::as_str)
        .filter(|status| matches!(*status, "NORMAL" | "ARCHIVED"))
        .unwrap_or(&user.row_status);
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
    let updated = get_user_by_id(env, user.id)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    json_response(json!({ "user": public_user(updated) }), 200).map_err(AppError::from)
}

async fn delete_user(
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if user.id == viewer.id {
        return Err(AppError::new(400, "Cannot delete yourself"));
    }
    db(env)?
        .prepare("DELETE FROM \"user\" WHERE id = ?")
        .bind(&[js_num(user.id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn get_user_setting(
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
    key: &str,
) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let value: Option<String> = db(env)?
        .prepare("SELECT value FROM user_setting WHERE user_id = ? AND key = ?")
        .bind(&[js_num(user.id), key.into()])?
        .first(Some("value"))
        .await?;
    json_response(
        json!({ "key": key, "value": value.unwrap_or_default() }),
        200,
    )
    .map_err(AppError::from)
}

async fn list_user_settings(
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let rows = db(env)?
        .prepare("SELECT key, value FROM user_setting WHERE user_id = ?")
        .bind(&[js_num(user.id)])?
        .all()
        .await?;
    let settings: Vec<Value> = rows.results()?;
    json_response(json!({ "settings": settings }), 200).map_err(AppError::from)
}

async fn update_user_setting(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
    key: &str,
) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let value = body
        .get("value")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    db(env)?.prepare("INSERT INTO user_setting (user_id, key, value) VALUES (?, ?, ?) ON CONFLICT (user_id, key) DO UPDATE SET value = excluded.value")
        .bind(&[js_num(user.id), key.into(), value.clone().into()])?
        .run()
        .await?;
    json_response(json!({ "key": key, "value": value }), 200).map_err(AppError::from)
}

async fn user_stats(
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let count: Option<i64> = db(env)?
        .prepare(
            "SELECT COUNT(*) AS count FROM memo WHERE creator_id = ? AND row_status = 'NORMAL'",
        )
        .bind(&[js_num(user.id)])?
        .first(Some("count"))
        .await?;
    let attachment_count: Option<i64> = db(env)?
        .prepare("SELECT COUNT(*) AS count FROM attachment WHERE creator_id = ?")
        .bind(&[js_num(user.id)])?
        .first(Some("count"))
        .await?;
    json_response(json!({ "stats": { "memoCount": count.unwrap_or(0), "attachmentCount": attachment_count.unwrap_or(0) } }), 200).map_err(AppError::from)
}

async fn list_access_tokens(
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let rows = db(env)?.prepare("SELECT id, name, token_prefix, created_ts, updated_ts, last_used_ts, expires_ts, row_status FROM user_access_token WHERE user_id = ? ORDER BY created_ts DESC")
        .bind(&[js_num(user.id)])?
        .all()
        .await?;
    let tokens: Vec<DbAccessToken> = rows.results()?;
    let payload: Vec<Value> = tokens
        .into_iter()
        .map(|token| {
            json!({
                "id": token.id,
                "name": token.name,
                "prefix": token.token_prefix,
                "createdTs": token.created_ts,
                "updatedTs": token.updated_ts,
                "lastUsedTs": token.last_used_ts,
                "expiresTs": token.expires_ts,
                "rowStatus": token.row_status
            })
        })
        .collect();
    json_response(json!({ "accessTokens": payload }), 200).map_err(AppError::from)
}

async fn create_access_token(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Unnamed Token")
        .trim();
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
    let id: Option<i64> = db(env)?
        .prepare("SELECT id FROM user_access_token WHERE token_hash = ?")
        .bind(&[token_hash.into()])?
        .first(Some("id"))
        .await?;
    json_response(
        json!({
            "accessToken": {
                "id": id.unwrap_or(0),
                "name": if name.is_empty() { "Unnamed Token" } else { name },
                "token": raw_token,
                "prefix": prefix,
                "createdTs": now,
                "expiresTs": expires_ts
            }
        }),
        201,
    )
    .map_err(AppError::from)
}

async fn delete_access_token(
    env: &Env,
    viewer: &Viewer,
    identifier: &str,
    token_id: &str,
) -> std::result::Result<Response, AppError> {
    let user = resolve_user(env, identifier)
        .await?
        .ok_or_else(|| AppError::new(404, "User not found"))?;
    if viewer.role != "ADMIN" && viewer.id != user.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    let id = token_id
        .parse::<i64>()
        .map_err(|_| AppError::new(400, "Invalid token ID"))?;
    let row: Option<Value> = db(env)?
        .prepare("SELECT id FROM user_access_token WHERE id = ? AND user_id = ?")
        .bind(&[js_num(id), js_num(user.id)])?
        .first(None)
        .await?;
    if row.is_none() {
        return Err(AppError::new(404, "Token not found"));
    }
    db(env)?
        .prepare("DELETE FROM user_access_token WHERE id = ?")
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
    let tags: Vec<Value> = counts
        .into_iter()
        .map(|(name, count)| json!({ "name": name, "count": count }))
        .collect();
    json_response(json!({ "tags": tags }), 200).map_err(AppError::from)
}

async fn rename_tag(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
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
        db(env)?
            .prepare("UPDATE memo SET content = ?, payload = ?, updated_ts = ? WHERE id = ?")
            .bind(&[
                next_content.clone().into(),
                build_memo_payload(&next_content).to_string().into(),
                js_num(unix_now()),
                js_num(memo.id),
            ])?
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

async fn get_recent_memos(
    env: &Env,
    viewer: &Viewer,
    limit: i64,
) -> std::result::Result<Vec<DbMemo>, AppError> {
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

async fn get_memos_by_uids(
    env: &Env,
    viewer: &Viewer,
    uids: &[String],
) -> std::result::Result<Vec<DbMemo>, AppError> {
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

async fn get_user_by_username(
    env: &Env,
    username: &str,
) -> std::result::Result<Option<DbUser>, AppError> {
    Ok(db(env)?
        .prepare("SELECT * FROM \"user\" WHERE username = ?")
        .bind(&[username.into()])?
        .first(None)
        .await?)
}

async fn get_user_by_id(env: &Env, id: i64) -> std::result::Result<Option<DbUser>, AppError> {
    Ok(db(env)?
        .prepare("SELECT * FROM \"user\" WHERE id = ?")
        .bind(&[js_num(id)])?
        .first(None)
        .await?)
}

async fn resolve_user(
    env: &Env,
    identifier: &str,
) -> std::result::Result<Option<DbUser>, AppError> {
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
    db(env)?
        .prepare(format!(
            "UPDATE attachment SET memo_id = NULL, updated_ts = ? WHERE memo_id IN ({})",
            placeholders
        ))
        .bind(&bind_with_first(unix_now(), ids))?
        .run()
        .await?;
    db(env)?
        .prepare(format!(
            "DELETE FROM reaction WHERE content_type = 'MEMO' AND content_id IN ({})",
            placeholders
        ))
        .bind(&values)?
        .run()
        .await?;
    db(env)?
        .prepare(format!(
            "DELETE FROM memo_share WHERE memo_id IN ({})",
            placeholders
        ))
        .bind(&values)?
        .run()
        .await?;
    let mut relation_values = values.clone();
    relation_values.extend(values.clone());
    db(env)?
        .prepare(format!(
            "DELETE FROM memo_relation WHERE memo_id IN ({}) OR related_memo_id IN ({})",
            placeholders, placeholders
        ))
        .bind(&relation_values)?
        .run()
        .await?;
    db(env)?
        .prepare(format!("DELETE FROM memo WHERE id IN ({})", placeholders))
        .bind(&values)?
        .run()
        .await?;
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

async fn memo_with_attachments(
    env: &Env,
    memo: DbMemo,
) -> std::result::Result<PublicMemo, AppError> {
    let attachments = list_attachments_for_memo(env, memo.id).await?;
    Ok(public_memo_with_attachments(memo, attachments))
}

async fn list_attachments_for_memo(
    env: &Env,
    memo_id: i64,
) -> std::result::Result<Vec<Value>, AppError> {
    let rows = db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id = ? ORDER BY attachment.created_ts, attachment.id")
        .bind(&[js_num(memo_id)])?
        .all()
        .await?;
    let attachments: Vec<DbAttachment> = rows.results()?;
    Ok(attachments.into_iter().map(public_attachment).collect())
}

fn shared_attachment_url(share_uid: &str, attachment_uid: &str, filename: &str) -> String {
    format!(
        "/api/v1/shares/{}/attachments/{}/{}",
        share_uid, attachment_uid, filename
    )
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
    let tags: Vec<String> = content
        .split_whitespace()
        .filter_map(|word| word.strip_prefix('#'))
        .map(|tag| {
            tag.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '/')
                .to_string()
        })
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
    value
        .trim()
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
    let first = content
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('#')
        .trim();
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
        Some(Value::Number(number)) => number
            .as_i64()
            .map(js_num)
            .unwrap_or_else(|| JsValue::from_f64(number.as_f64().unwrap_or_default())),
        Some(Value::Bool(value)) => js_num(if *value { 1 } else { 0 }),
        Some(Value::Null) | None => JsValue::NULL,
        Some(other) => other.to_string().into(),
    }
}

fn normalize_http_url(
    value: impl AsRef<str>,
    message: &str,
) -> std::result::Result<String, AppError> {
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

async fn record_audit(
    env: &Env,
    viewer: Option<&Viewer>,
    action: &str,
    target: &str,
    detail: Value,
) {
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
    db(env)?
        .prepare("CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_ts)")
        .run()
        .await?;
    db(env)?
        .prepare("CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action, created_ts)")
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
mod tests;

include!("support.rs");
