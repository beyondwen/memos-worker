use super::*;

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
