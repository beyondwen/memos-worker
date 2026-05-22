use super::*;

pub(crate) async fn restore_backup(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let payload = read_backup_payload(req, env).await?;
    validate_backup_payload(&payload)?;
    let preview = backup_preview(&payload);
    let restore_point = create_backup_artifact(env).await?;
    if let Err(error) = apply_backup_payload(env, &payload).await {
        record_audit(
            env,
            Some(viewer),
            "backup.restore_failed",
            "backup",
            json!({
                "stage": error.stage,
                "error": error.error.message,
                "restorePoint": backup_artifact_payload(&restore_point)
            }),
        )
        .await;
        return Err(AppError::new(
            error.error.status,
            format!(
                "Backup restore failed at {}: {}",
                error.stage, error.error.message
            ),
        ));
    }
    record_audit(
        env,
        Some(viewer),
        "backup.restore",
        "backup",
        json!({
            "preview": preview,
            "restorePoint": backup_artifact_payload(&restore_point)
        }),
    )
    .await;
    json_response(
        json!({
            "restored": preview,
            "restorePoint": backup_artifact_payload(&restore_point)
        }),
        200,
    )
    .map_err(AppError::from)
}

async fn apply_backup_payload(env: &Env, payload: &Value) -> std::result::Result<(), RestoreError> {
    restore_memos(env, payload)
        .await
        .map_err(|error| RestoreError {
            stage: "memos",
            error,
        })?;
    restore_attachments(env, payload)
        .await
        .map_err(|error| RestoreError {
            stage: "attachments",
            error,
        })?;
    restore_relations(env, payload)
        .await
        .map_err(|error| RestoreError {
            stage: "relations",
            error,
        })?;
    Ok(())
}

struct RestoreError {
    stage: &'static str,
    error: AppError,
}

async fn restore_memos(env: &Env, payload: &Value) -> std::result::Result<(), AppError> {
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
            if let Some(id) = item.get("id").and_then(Value::as_i64) {
                sync_memo_index_fields(
                    env,
                    id,
                    item.get("content").and_then(Value::as_str).unwrap_or(""),
                    item.get("payload").and_then(Value::as_str).unwrap_or("{}"),
                    item.get("updated_ts")
                        .or_else(|| item.get("updatedTs"))
                        .and_then(Value::as_i64)
                        .unwrap_or_else(unix_now),
                )
                .await?;
            }
        }
    }
    Ok(())
}

async fn restore_attachments(env: &Env, payload: &Value) -> std::result::Result<(), AppError> {
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
    Ok(())
}

async fn restore_relations(env: &Env, payload: &Value) -> std::result::Result<(), AppError> {
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
    Ok(())
}

pub(crate) fn validate_backup_payload(payload: &Value) -> std::result::Result<(), AppError> {
    for key in ["users", "memos", "attachments", "relations"] {
        if payload.get(key).and_then(Value::as_array).is_none() {
            return Err(AppError::new(400, "Invalid backup payload"));
        }
    }
    validate_items_have_i64(
        payload,
        "memos",
        &["id", "creator_id", "created_ts", "updated_ts"],
    )?;
    validate_items_have_i64(
        payload,
        "attachments",
        &["id", "creator_id", "created_ts", "updated_ts"],
    )?;
    validate_items_have_i64(payload, "relations", &["memo_id", "related_memo_id"])?;
    Ok(())
}

fn validate_items_have_i64(
    payload: &Value,
    array_key: &str,
    fields: &[&str],
) -> std::result::Result<(), AppError> {
    let Some(items) = payload.get(array_key).and_then(Value::as_array) else {
        return Err(AppError::new(400, "Invalid backup payload"));
    };
    for item in items {
        for field in fields {
            if item.get(*field).and_then(Value::as_i64).is_none() {
                return Err(AppError::new(400, "Invalid backup payload"));
            }
        }
    }
    Ok(())
}
