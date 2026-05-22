use super::*;

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
    let raw_token = format!("memos_pat_{}", base64url(&random_bytes(18)?));
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
