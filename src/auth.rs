use super::*;

pub(crate) async fn get_instance(env: &Env) -> std::result::Result<Response, AppError> {
    let db = db(env)?;
    let count: Option<i64> = db
        .prepare("SELECT COUNT(*) AS count FROM \"user\"")
        .first(Some("count"))
        .await?;
    json_response(
        json!({
            "name": "Memos Worker",
            "setupRequired": count.unwrap_or(0) == 0,
            "signupEnabled": signup_enabled(env)
        }),
        200,
    )
    .map_err(AppError::from)
}

pub(crate) async fn setup_admin(
    req: &mut Request,
    env: &Env,
) -> std::result::Result<Response, AppError> {
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
    enforce_auth_rate_limit(env, req, "setup", &username).await?;
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Password is required"))?;
    assert_password(password)?;
    let password_hash = hash_password(password)?;
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
    reset_auth_rate_limit(env, "setup", &auth_rate_limit_actor(req, &username)).await;
    create_auth_response(env, req, user, 201).await
}

pub(crate) async fn sign_up(
    req: &mut Request,
    env: &Env,
) -> std::result::Result<Response, AppError> {
    if !signup_enabled(env) {
        return Err(AppError::new(403, "Public signup is disabled"));
    }
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let username = normalize_username(body.get("username").and_then(Value::as_str))?;
    enforce_auth_rate_limit(env, req, "signup", &username).await?;
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Password is required"))?;
    assert_password(password)?;
    if get_user_by_username(env, &username).await?.is_some() {
        record_auth_rate_limit_failure(env, "signup", &auth_rate_limit_actor(req, &username)).await;
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
            hash_password(password)?.into(),
        ])?
        .run()
        .await?;
    let user = get_user_by_username(env, &username)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create user"))?;
    reset_auth_rate_limit(env, "signup", &auth_rate_limit_actor(req, &username)).await;
    create_auth_response(env, req, user, 201).await
}

pub(crate) async fn sign_in(
    req: &mut Request,
    env: &Env,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let username = normalize_username(body.get("username").and_then(Value::as_str))?;
    enforce_auth_rate_limit(env, req, "signin", &username).await?;
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Password is required"))?;
    let user = match get_user_by_username(env, &username).await? {
        Some(user) => user,
        None => {
            record_auth_rate_limit_failure(env, "signin", &auth_rate_limit_actor(req, &username))
                .await;
            return Err(AppError::new(401, "Invalid username or password"));
        }
    };
    if user.row_status != "NORMAL" || !verify_password(password, &user.password_hash) {
        record_auth_rate_limit_failure(env, "signin", &auth_rate_limit_actor(req, &username)).await;
        return Err(AppError::new(401, "Invalid username or password"));
    }
    reset_auth_rate_limit(env, "signin", &auth_rate_limit_actor(req, &username)).await;
    create_auth_response(env, req, user, 200).await
}

pub(crate) async fn refresh_session(
    req: &Request,
    env: &Env,
) -> std::result::Result<Response, AppError> {
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
    let session_id = claims
        .tid
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new(401, "Invalid refresh token"))?;
    let user_id = claims
        .sub
        .parse::<i64>()
        .map_err(|_| AppError::new(401, "Invalid token subject"))?;
    validate_refresh_session(env, session_id, user_id, refresh).await?;
    let user = get_user_by_id(env, user_id)
        .await?
        .ok_or_else(|| AppError::new(401, "User unavailable"))?;
    if user.row_status != "NORMAL" {
        return Err(AppError::new(401, "User unavailable"));
    }
    revoke_session(env, session_id).await?;
    create_auth_response(env, req, user, 200).await
}

pub(crate) async fn sign_out(req: &Request, env: &Env) -> std::result::Result<Response, AppError> {
    validate_csrf(req, &Method::Post)?;
    if let Some(session_id) = refresh_session_id_from_request(req, env) {
        revoke_session(env, &session_id).await?;
    }
    let mut res = json_response(json!({ "ok": true }), 200).map_err(AppError::from)?;
    append_cookie(&mut res, &clear_cookie(REFRESH_COOKIE));
    append_cookie(&mut res, &clear_cookie(ACCESS_COOKIE));
    append_cookie(&mut res, &clear_cookie_at_path(CSRF_COOKIE, "/", false));
    Ok(res)
}

pub(crate) async fn create_auth_response(
    env: &Env,
    req: &Request,
    user: DbUser,
    status: u16,
) -> std::result::Result<Response, AppError> {
    let now = unix_now();
    let session_id = generate_uid("s")?;
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
    let csrf_token = generate_uid("csrf")?;
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
    append_cookie(
        &mut res,
        &cookie(ACCESS_COOKIE, &access_token, ACCESS_TTL, true),
    );
    append_cookie(
        &mut res,
        &cookie_at_path(CSRF_COOKIE, &csrf_token, REFRESH_TTL, false, "/"),
    );
    Ok(res)
}

pub(crate) async fn list_sessions(
    req: &Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let current = access_session_id_from_request(req, env);
    let rows = db(env)?
        .prepare("SELECT id, created_ts, updated_ts, last_used_ts, expires_ts, user_agent, row_status FROM user_session WHERE user_id = ? ORDER BY updated_ts DESC, created_ts DESC")
        .bind(&[js_num(viewer.id)])?
        .all()
        .await?;
    let sessions: Vec<Value> = rows
        .results::<Value>()?
        .into_iter()
        .map(|row| {
            let id = row
                .get("id")
                .or_else(|| row.get("ID"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            json!({
                "id": id,
                "createdTs": row_value_i64(&row, "created_ts"),
                "updatedTs": row_value_i64(&row, "updated_ts"),
                "lastUsedTs": row_value_i64_opt(&row, "last_used_ts"),
                "expiresTs": row_value_i64(&row, "expires_ts"),
                "userAgent": row_value_str(&row, "user_agent"),
                "rowStatus": row_value_str(&row, "row_status"),
                "current": current.as_deref() == Some(id.as_str())
            })
        })
        .collect();
    json_response(json!({ "sessions": sessions }), 200).map_err(AppError::from)
}

pub(crate) async fn revoke_session_route(
    env: &Env,
    viewer: &Viewer,
    session_id: &str,
) -> std::result::Result<Response, AppError> {
    let owned: Option<i64> = db(env)?
        .prepare("SELECT COUNT(*) AS count FROM user_session WHERE id = ? AND user_id = ?")
        .bind(&[session_id.into(), js_num(viewer.id)])?
        .first(Some("count"))
        .await?;
    if owned.unwrap_or(0) != 1 {
        return Err(AppError::new(404, "Session not found"));
    }
    revoke_session(env, session_id).await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

pub(crate) async fn current_viewer(
    req: &Request,
    env: &Env,
) -> std::result::Result<Viewer, AppError> {
    let mut token = None;
    if let Ok(Some(auth)) = req.headers().get("Authorization") {
        if let Some(rest) = auth.strip_prefix("Bearer ") {
            token = Some(rest.trim().to_string());
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
    let session_id = claims
        .tid
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    validate_access_session(env, session_id, id).await?;
    let user = get_user_by_id(env, id)
        .await?
        .ok_or_else(|| AppError::new(401, "Unauthorized"))?;
    if user.row_status != "NORMAL" {
        return Err(AppError::new(401, "Unauthorized"));
    }
    Ok(Viewer {
        id: user.id,
        role: user.role,
    })
}

pub(crate) async fn viewer_from_access_token(
    env: &Env,
    token: &str,
) -> std::result::Result<Viewer, AppError> {
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
    if user.row_status != "NORMAL" {
        return Err(AppError::new(401, "Unauthorized"));
    }
    Ok(Viewer {
        id: user.id,
        role: user.role,
    })
}

pub(crate) async fn validate_refresh_session(
    env: &Env,
    session_id: &str,
    user_id: i64,
    refresh_token: &str,
) -> std::result::Result<(), AppError> {
    let token_hash = sha256_hex(refresh_token);
    let count: Option<i64> = db(env)?
        .prepare("SELECT COUNT(*) AS count FROM user_session WHERE id = ? AND user_id = ? AND refresh_token_hash = ? AND row_status = 'NORMAL' AND expires_ts > ?")
        .bind(&[
            session_id.into(),
            js_num(user_id),
            token_hash.into(),
            js_num(unix_now()),
        ])?
        .first(Some("count"))
        .await?;
    if count.unwrap_or(0) == 1 {
        Ok(())
    } else {
        Err(AppError::new(401, "Invalid refresh token"))
    }
}

pub(crate) async fn validate_access_session(
    env: &Env,
    session_id: &str,
    user_id: i64,
) -> std::result::Result<(), AppError> {
    let count: Option<i64> = db(env)?
        .prepare("SELECT COUNT(*) AS count FROM user_session WHERE id = ? AND user_id = ? AND row_status = 'NORMAL' AND expires_ts > ?")
        .bind(&[session_id.into(), js_num(user_id), js_num(unix_now())])?
        .first(Some("count"))
        .await?;
    if count.unwrap_or(0) == 1 {
        db(env)?
            .prepare("UPDATE user_session SET last_used_ts = ?, updated_ts = ? WHERE id = ?")
            .bind(&[js_num(unix_now()), js_num(unix_now()), session_id.into()])?
            .run()
            .await?;
        Ok(())
    } else {
        Err(AppError::new(401, "Unauthorized"))
    }
}

pub(crate) async fn revoke_session(
    env: &Env,
    session_id: &str,
) -> std::result::Result<(), AppError> {
    db(env)?
        .prepare("UPDATE user_session SET row_status = 'REVOKED', updated_ts = ? WHERE id = ?")
        .bind(&[js_num(unix_now()), session_id.into()])?
        .run()
        .await?;
    Ok(())
}

pub(crate) async fn revoke_user_sessions(
    env: &Env,
    user_id: i64,
) -> std::result::Result<(), AppError> {
    db(env)?
        .prepare("UPDATE user_session SET row_status = 'REVOKED', updated_ts = ? WHERE user_id = ?")
        .bind(&[js_num(unix_now()), js_num(user_id)])?
        .run()
        .await?;
    Ok(())
}

pub(crate) async fn prune_auth_records(
    env: &Env,
    retention_days: i64,
) -> std::result::Result<i64, AppError> {
    ensure_auth_rate_limit_table(env).await?;
    let now = unix_now();
    let cutoff = auth_record_retention_cutoff(now, retention_days);
    let expired_sessions = delete_and_count(
        env,
        "DELETE FROM user_session WHERE expires_ts < ?",
        &[js_num(now)],
    )
    .await?;
    let revoked_sessions = delete_and_count(
        env,
        "DELETE FROM user_session WHERE row_status = 'REVOKED' AND updated_ts < ?",
        &[js_num(cutoff)],
    )
    .await?;
    let expired_tokens = delete_and_count(
        env,
        "DELETE FROM user_access_token WHERE expires_ts IS NOT NULL AND expires_ts < ?",
        &[js_num(now)],
    )
    .await?;
    let revoked_tokens = delete_and_count(
        env,
        "DELETE FROM user_access_token WHERE row_status = 'REVOKED' AND updated_ts < ?",
        &[js_num(cutoff)],
    )
    .await?;
    let old_rate_limits = delete_and_count(
        env,
        "DELETE FROM auth_rate_limit WHERE updated_ts < ?",
        &[js_num(cutoff)],
    )
    .await
    .unwrap_or(0);
    Ok(expired_sessions + revoked_sessions + expired_tokens + revoked_tokens + old_rate_limits)
}

async fn delete_and_count(
    env: &Env,
    sql: &str,
    values: &[JsValue],
) -> std::result::Result<i64, AppError> {
    db(env)?.prepare(sql).bind(values)?.run().await?;
    let count: Option<i64> = db(env)?
        .prepare("SELECT changes() AS count")
        .first(Some("count"))
        .await?;
    Ok(count.unwrap_or(0))
}

pub(crate) fn auth_record_retention_cutoff(now: i64, retention_days: i64) -> i64 {
    now - retention_days.max(0) * 24 * 60 * 60
}

pub(crate) fn validate_csrf(req: &Request, method: &Method) -> std::result::Result<(), AppError> {
    let cookie_header = req
        .headers()
        .get("Cookie")
        .ok()
        .flatten()
        .unwrap_or_default();
    if !csrf_required_for_request(method, &cookie_header) {
        return Ok(());
    }
    let cookies = parse_cookies(&cookie_header);
    let cookie_token = cookies.get(CSRF_COOKIE).map(String::as_str);
    let header_token = req
        .headers()
        .get(CSRF_HEADER)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string());
    if csrf_tokens_match(header_token.as_deref(), cookie_token) {
        Ok(())
    } else {
        Err(AppError::new(403, "Invalid CSRF token"))
    }
}

pub(crate) fn csrf_required_for_request(method: &Method, cookie_header: &str) -> bool {
    if matches!(method, Method::Get | Method::Head | Method::Options) {
        return false;
    }
    let cookies = parse_cookies(cookie_header);
    cookies.contains_key(ACCESS_COOKIE) || cookies.contains_key(REFRESH_COOKIE)
}

pub(crate) fn csrf_tokens_match(header: Option<&str>, cookie: Option<&str>) -> bool {
    let Some(header) = header.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let Some(cookie) = cookie.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    constant_time_equal(header.as_bytes(), cookie.as_bytes())
}

pub(crate) async fn enforce_auth_rate_limit(
    env: &Env,
    req: &Request,
    scope: &str,
    username: &str,
) -> std::result::Result<(), AppError> {
    ensure_auth_rate_limit_table(env).await?;
    let actor = auth_rate_limit_actor(req, username);
    let row: Option<Value> = db(env)?
        .prepare("SELECT window_start_ts, attempts, blocked_until_ts FROM auth_rate_limit WHERE scope = ? AND actor_key = ?")
        .bind(&[scope.into(), actor.into()])?
        .first(None)
        .await?;
    let now = unix_now();
    if let Some(row) = row {
        let blocked_until = row
            .get("blocked_until_ts")
            .or_else(|| row.get("BLOCKED_UNTIL_TS"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if blocked_until > now {
            return Err(AppError::new(429, "Too many attempts, try again later"));
        }
    }
    Ok(())
}

pub(crate) async fn record_auth_rate_limit_failure(env: &Env, scope: &str, actor: &str) {
    if ensure_auth_rate_limit_table(env).await.is_err() {
        return;
    }
    let now = unix_now();
    let row: Option<Value> = if let Ok(database) = db(env) {
        match database
            .prepare("SELECT window_start_ts, attempts FROM auth_rate_limit WHERE scope = ? AND actor_key = ?")
            .bind(&[scope.into(), actor.into()])
        {
            Ok(stmt) => stmt.first(None).await.ok().flatten(),
            Err(_) => None,
        }
    } else {
        None
    };
    let (window_start, attempts) = row
        .map(|value| {
            let window_start = value
                .get("window_start_ts")
                .or_else(|| value.get("WINDOW_START_TS"))
                .and_then(Value::as_i64)
                .unwrap_or(now);
            let attempts = value
                .get("attempts")
                .or_else(|| value.get("ATTEMPTS"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            (window_start, attempts)
        })
        .unwrap_or((now, 0));
    let window_start = if now - window_start > 10 * 60 {
        now
    } else {
        window_start
    };
    let attempts = if window_start == now { 1 } else { attempts + 1 };
    let blocked_until = if attempts >= 5 { now + 15 * 60 } else { 0 };
    if let Ok(database) = db(env) {
        if let Ok(stmt) = database
            .prepare("INSERT OR REPLACE INTO auth_rate_limit (scope, actor_key, window_start_ts, attempts, blocked_until_ts, updated_ts) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&[
                scope.into(),
                actor.into(),
                js_num(window_start),
                js_num(attempts),
                js_num(blocked_until),
                js_num(now),
            ])
        {
            let _ = stmt.run().await;
        }
    }
}

pub(crate) async fn reset_auth_rate_limit(env: &Env, scope: &str, actor: &str) {
    if let Ok(database) = db(env) {
        if let Ok(stmt) = database
            .prepare("DELETE FROM auth_rate_limit WHERE scope = ? AND actor_key = ?")
            .bind(&[scope.into(), actor.into()])
        {
            let _ = stmt.run().await;
        }
    }
}

pub(crate) async fn ensure_auth_rate_limit_table(env: &Env) -> std::result::Result<(), AppError> {
    db(env)?.prepare("CREATE TABLE IF NOT EXISTS auth_rate_limit (scope TEXT NOT NULL, actor_key TEXT NOT NULL, window_start_ts INTEGER NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, blocked_until_ts INTEGER NOT NULL DEFAULT 0, updated_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), PRIMARY KEY (scope, actor_key))")
        .run()
        .await?;
    Ok(())
}

pub(crate) fn auth_rate_limit_actor(req: &Request, username: &str) -> String {
    format!("{}:{}", request_ip(req), username.trim().to_lowercase())
}

pub(crate) fn request_ip(req: &Request) -> String {
    for header in ["CF-Connecting-IP", "X-Forwarded-For", "X-Real-IP"] {
        if let Ok(Some(value)) = req.headers().get(header) {
            let ip = value.split(',').next().unwrap_or("").trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }
    "unknown".to_string()
}

fn refresh_session_id_from_request(req: &Request, env: &Env) -> Option<String> {
    let cookies = parse_cookies(
        req.headers()
            .get("Cookie")
            .ok()
            .flatten()
            .unwrap_or_default()
            .as_str(),
    );
    let refresh = cookies.get(REFRESH_COOKIE)?;
    let claims = verify_jwt(refresh, &server_secret(env).ok()?).ok()?;
    if claims.token_type == "refresh" {
        claims.tid
    } else {
        None
    }
}

fn access_session_id_from_request(req: &Request, env: &Env) -> Option<String> {
    let mut token = None;
    if let Ok(Some(auth)) = req.headers().get("Authorization") {
        if let Some(rest) = auth.strip_prefix("Bearer ") {
            token = Some(rest.trim().to_string());
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
    let claims = verify_jwt(&token?, &server_secret(env).ok()?).ok()?;
    if claims.token_type == "access" {
        claims.tid
    } else {
        None
    }
}

fn row_value_str(row: &Value, key: &str) -> String {
    row.get(key)
        .or_else(|| row.get(&key.to_ascii_uppercase()))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn row_value_i64(row: &Value, key: &str) -> i64 {
    row_value_i64_opt(row, key).unwrap_or(0)
}

fn row_value_i64_opt(row: &Value, key: &str) -> Option<i64> {
    row.get(key)
        .or_else(|| row.get(&key.to_ascii_uppercase()))
        .and_then(Value::as_i64)
}

pub(crate) fn signup_enabled(env: &Env) -> bool {
    let value = env_text(env, "ALLOW_SIGNUP").or_else(|| env_text(env, "SIGNUP_ENABLED"));
    is_truthy_env(value.as_deref())
}

pub(crate) fn is_truthy_env(value: Option<&str>) -> bool {
    matches!(
        value.unwrap_or("").trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
