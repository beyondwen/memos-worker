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
            hash_password(new_password)?.into(),
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
