use super::*;

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
