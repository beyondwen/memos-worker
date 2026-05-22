use super::*;

pub(crate) async fn create_backup(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let artifact = create_backup_artifact(env).await?;
    record_audit(
        env,
        Some(viewer),
        "backup.create",
        &artifact.key,
        json!({ "size": artifact.size }),
    )
    .await;
    json_response(backup_artifact_payload(&artifact), 201).map_err(AppError::from)
}

pub(crate) async fn create_scheduled_backup(
    env: &Env,
) -> std::result::Result<BackupArtifact, AppError> {
    let artifact = create_backup_artifact(env).await?;
    record_audit(
        env,
        None,
        "backup.create",
        &artifact.key,
        json!({ "size": artifact.size, "source": "scheduled" }),
    )
    .await;
    Ok(artifact)
}

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

pub(crate) async fn list_backups(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let listed = env
        .bucket("MEMOS_BUCKET")?
        .list()
        .prefix("backups/")
        .execute()
        .await?;
    let mut backups: Vec<Value> = listed
        .objects()
        .into_iter()
        .map(|object| {
            json!({
                "key": object.key(),
                "size": object.size(),
                "uploaded": object.uploaded().to_string()
            })
        })
        .collect();
    backups.sort_by(|a, b| {
        b.get("uploaded")
            .and_then(Value::as_str)
            .cmp(&a.get("uploaded").and_then(Value::as_str))
    });
    json_response(json!({ "backups": backups }), 200).map_err(AppError::from)
}

pub(crate) async fn download_backup(
    env: &Env,
    url: &Url,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let key = url
        .query_pairs()
        .find(|(name, _)| name == "key")
        .map(|(_, value)| value.to_string())
        .unwrap_or_default();
    if !key.starts_with("backups/") {
        return Err(AppError::new(400, "Invalid backup key"));
    }
    let object = env
        .bucket("MEMOS_BUCKET")?
        .get(key.clone())
        .execute()
        .await?
        .ok_or_else(|| AppError::new(404, "Backup not found"))?;
    let body = object
        .body()
        .ok_or_else(|| AppError::new(404, "Backup not found"))?
        .response_body()?;
    let filename = key.rsplit('/').next().unwrap_or("memos-backup.json");
    let mut response = ResponseBuilder::new().body(body);
    response
        .headers_mut()
        .set("Content-Type", "application/json")?;
    response.headers_mut().set(
        "Content-Disposition",
        &format!("attachment; filename=\"{}\"", filename),
    )?;
    Ok(response)
}

pub(crate) async fn preview_backup(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let payload = read_backup_payload(req, env).await?;
    json_response(json!({ "preview": backup_preview(&payload) }), 200).map_err(AppError::from)
}

pub(crate) async fn restore_backup(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
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
    record_audit(
        env,
        Some(viewer),
        "backup.restore",
        "backup",
        preview.clone(),
    )
    .await;
    json_response(json!({ "restored": preview }), 200).map_err(AppError::from)
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

pub(crate) fn backup_key() -> String {
    let stamp = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| unix_now().to_string())
        .replace([':', '.'], "-");
    format!("backups/memos-{}.json", stamp)
}
