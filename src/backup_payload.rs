use super::*;

pub(crate) async fn create_backup_artifact(
    env: &Env,
) -> std::result::Result<BackupArtifact, AppError> {
    let key = backup_key();
    let body = serde_json::to_string_pretty(&build_backup_payload(env).await?)
        .map_err(|error| AppError::new(500, error.to_string()))?;
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

pub(crate) fn backup_artifact_payload(artifact: &BackupArtifact) -> Value {
    json!({ "backup": { "key": artifact.key, "size": artifact.size } })
}

pub(crate) async fn read_backup_payload(
    req: &mut Request,
    env: &Env,
) -> std::result::Result<Value, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    if let Some(payload) = body.get("payload") {
        return Ok(payload.clone());
    }
    let key = body.get("key").and_then(Value::as_str).unwrap_or("");
    if !key.starts_with("backups/") {
        return Err(AppError::new(400, "Invalid backup key"));
    }
    let object = env
        .bucket("MEMOS_BUCKET")?
        .get(key.to_string())
        .execute()
        .await?
        .ok_or_else(|| AppError::new(404, "Backup not found"))?;
    let text = object
        .body()
        .ok_or_else(|| AppError::new(404, "Backup not found"))?
        .text()
        .await?;
    serde_json::from_str(&text).map_err(|_| AppError::new(400, "Invalid backup payload"))
}

pub(crate) async fn build_backup_payload(env: &Env) -> std::result::Result<Value, AppError> {
    let users = db(env)?.prepare("SELECT id, created_ts, updated_ts, row_status, username, role, email, nickname, avatar_url, description FROM \"user\" ORDER BY id").all().await?.results::<Value>()?;
    let memos = db(env)?
        .prepare("SELECT * FROM memo ORDER BY created_ts, id")
        .all()
        .await?
        .results::<Value>()?;
    let attachments = db(env)?.prepare("SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload FROM attachment ORDER BY created_ts, id").all().await?.results::<Value>()?;
    let relations = db(env)?
        .prepare("SELECT * FROM memo_relation ORDER BY memo_id, related_memo_id")
        .all()
        .await?
        .results::<Value>()?;
    Ok(json!({
        "exportedAt": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default(),
        "users": users,
        "memos": memos,
        "attachments": attachments,
        "relations": relations
    }))
}

pub(crate) fn backup_preview(payload: &Value) -> Value {
    json!({
        "userCount": payload.get("users").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "memoCount": payload.get("memos").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "attachmentCount": payload.get("attachments").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "relationCount": payload.get("relations").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
    })
}

fn backup_key() -> String {
    let stamp = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| unix_now().to_string())
        .replace([':', '.'], "-");
    format!("backups/memos-{}.json", stamp)
}
