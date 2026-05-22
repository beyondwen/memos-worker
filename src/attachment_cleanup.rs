use super::*;

pub(crate) async fn batch_delete_attachments(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let uids: Vec<String> = body
        .get("attachmentUids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|uid| !uid.is_empty())
                .map(ToString::to_string)
                .take(100)
                .collect()
        })
        .unwrap_or_default();
    let rows = if uids.is_empty() && viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL LIMIT 100")
            .all()
            .await?
    } else if uids.is_empty() {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.creator_id = ? LIMIT 100")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    } else {
        let placeholders = placeholders(uids.len());
        let mut values: Vec<JsValue> = uids.iter().map(|uid| uid.clone().into()).collect();
        let owner_sql = if viewer.role == "ADMIN" {
            ""
        } else {
            values.push(js_num(viewer.id));
            " AND attachment.creator_id = ?"
        };
        db(env)?.prepare(format!("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.uid IN ({}){}", placeholders, owner_sql))
            .bind(&values)?
            .all()
            .await?
    };
    let attachments: Vec<DbAttachment> = rows.results()?;
    let mut deleted = 0;
    let mut size = 0;
    for attachment in attachments {
        let _ = env
            .bucket("MEMOS_BUCKET")?
            .delete(attachment.reference.clone())
            .await;
        db(env)?
            .prepare("DELETE FROM attachment WHERE id = ?")
            .bind(&[js_num(attachment.id)])?
            .run()
            .await?;
        deleted += 1;
        size += attachment.size;
    }
    json_response(json!({ "deleted": deleted, "size": size }), 200).map_err(AppError::from)
}
