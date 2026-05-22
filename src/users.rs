use super::*;

pub(crate) async fn update_me(
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

pub(crate) async fn change_password(
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

pub(crate) async fn list_users(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let rows = db(env)?
        .prepare("SELECT * FROM \"user\" ORDER BY id")
        .all()
        .await?;
    let users: Vec<DbUser> = rows.results()?;
    let payload: Vec<PublicUser> = users.into_iter().map(public_user).collect();
    json_response(json!({ "users": payload }), 200).map_err(AppError::from)
}

pub(crate) async fn user_subroute(
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

pub(crate) fn parse_user_settings_path(path: &str) -> Option<(&str, Option<&str>)> {
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

pub(crate) async fn update_user(
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

pub(crate) async fn delete_user(
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

pub(crate) async fn get_user_setting(
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

pub(crate) async fn list_user_settings(
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

pub(crate) async fn update_user_setting(
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

pub(crate) async fn user_stats(
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

pub(crate) async fn list_access_tokens(
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

pub(crate) async fn create_access_token(
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

pub(crate) async fn delete_access_token(
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
