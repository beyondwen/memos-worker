use super::*;

pub(crate) async fn get_instance(env: &Env) -> std::result::Result<Response, AppError> {
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
    create_auth_response(env, req, user, 201).await
}

pub(crate) async fn sign_up(
    req: &mut Request,
    env: &Env,
) -> std::result::Result<Response, AppError> {
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
            hash_password(password)?.into(),
        ])?
        .run()
        .await?;
    let user = get_user_by_username(env, &username)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create user"))?;
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
    let user_id = claims
        .sub
        .parse::<i64>()
        .map_err(|_| AppError::new(401, "Invalid token subject"))?;
    let user = get_user_by_id(env, user_id)
        .await?
        .ok_or_else(|| AppError::new(401, "User unavailable"))?;
    create_auth_response(env, req, user, 200).await
}

pub(crate) fn sign_out() -> std::result::Result<Response, AppError> {
    let mut res = json_response(json!({ "ok": true }), 200).map_err(AppError::from)?;
    append_cookie(&mut res, &clear_cookie(REFRESH_COOKIE));
    append_cookie(&mut res, &clear_cookie(ACCESS_COOKIE));
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
    Ok(Viewer {
        id: user.id,
        role: user.role,
    })
}
