use super::*;

const SCHEDULED_BACKUP_RETENTION: usize = 14;

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
        json!({ "size": artifact.size, "encrypted": artifact.encrypted, "keyId": artifact.key_id }),
    )
    .await;
    json_response(backup_artifact_payload(&artifact), 201).map_err(AppError::from)
}

pub(crate) async fn create_scheduled_backup(
    env: &Env,
) -> std::result::Result<BackupArtifact, AppError> {
    let artifact = create_backup_artifact(env).await?;
    let pruned = prune_old_backups(env, SCHEDULED_BACKUP_RETENTION).await?;
    record_audit(
        env,
        None,
        "backup.create",
        &artifact.key,
        json!({ "size": artifact.size, "source": "scheduled", "pruned": pruned, "encrypted": artifact.encrypted, "keyId": artifact.key_id }),
    )
    .await;
    Ok(artifact)
}

pub(crate) fn backup_keys_to_prune(
    mut backups: Vec<(String, String)>,
    retain: usize,
) -> Vec<String> {
    if backups.len() <= retain {
        return Vec::new();
    }
    backups.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let prune_count = backups.len().saturating_sub(retain);
    backups
        .into_iter()
        .take(prune_count)
        .map(|(key, _)| key)
        .collect()
}

async fn prune_old_backups(env: &Env, retain: usize) -> std::result::Result<usize, AppError> {
    let bucket = env.bucket("MEMOS_BUCKET")?;
    let listed = bucket.list().prefix("backups/").execute().await?;
    let backups = listed
        .objects()
        .into_iter()
        .map(|object| (object.key(), object.uploaded().to_string()))
        .collect();
    let keys = backup_keys_to_prune(backups, retain);
    let pruned = keys.len();
    for key in keys {
        let _ = bucket.delete(key).await;
    }
    Ok(pruned)
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
    let text = object
        .body()
        .ok_or_else(|| AppError::new(404, "Backup not found"))?
        .text()
        .await?;
    let body = backup_plaintext(env, &text).await?;
    let filename = key.rsplit('/').next().unwrap_or("memos-backup.json");
    let mut response = Response::ok(body)?;
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
