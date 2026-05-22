use super::*;

pub(crate) async fn system_health(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let memo_index = memo_index_health(env).await?;
    let backup = backup_health(env).await?;
    let healthy =
        memo_index.healthy && backup.get("r2Available").and_then(Value::as_bool) == Some(true);
    json_response(
        json!({
            "status": if healthy { "healthy" } else { "degraded" },
            "checkedTs": unix_now(),
            "memoIndex": memo_index.to_json(),
            "backup": backup
        }),
        200,
    )
    .map_err(AppError::from)
}

async fn backup_health(env: &Env) -> std::result::Result<Value, AppError> {
    let encryption = backup_encryption_status(env);
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
    let latest = backups.first().cloned().unwrap_or(Value::Null);
    Ok(json!({
        "r2Available": true,
        "count": backups.len(),
        "latest": latest,
        "encryption": encryption
    }))
}
