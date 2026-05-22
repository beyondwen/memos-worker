use super::*;

pub(crate) async fn list_inbox(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let rows = db(env)?.prepare("SELECT inbox.*, \"user\".username AS sender_username, \"user\".nickname AS sender_nickname FROM inbox LEFT JOIN \"user\" ON \"user\".id = inbox.sender_id WHERE inbox.receiver_id = ? ORDER BY inbox.created_ts DESC LIMIT 100")
        .bind(&[js_num(viewer.id)])?
        .all()
        .await?;
    let inbox: Vec<DbInboxRow> = rows.results()?;
    let unread_count: Option<i64> = db(env)?
        .prepare("SELECT COUNT(*) AS count FROM inbox WHERE receiver_id = ? AND status = 'UNREAD'")
        .bind(&[js_num(viewer.id)])?
        .first(Some("count"))
        .await?;
    let payload: Vec<Value> = inbox.into_iter().map(public_inbox_item).collect();
    json_response(
        json!({ "inbox": payload, "unreadCount": unread_count.unwrap_or(0) }),
        200,
    )
    .map_err(AppError::from)
}

pub(crate) async fn update_inbox_status(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let status = if body.get("status").and_then(Value::as_str) == Some("READ") {
        "READ"
    } else {
        "UNREAD"
    };
    let ids: Vec<i64> = body
        .get("ids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_i64)
                .filter(|id| *id > 0)
                .collect()
        })
        .unwrap_or_default();

    if ids.is_empty() {
        db(env)?
            .prepare("UPDATE inbox SET status = ? WHERE receiver_id = ?")
            .bind(&[status.into(), js_num(viewer.id)])?
            .run()
            .await?;
    } else {
        let mut values: Vec<JsValue> = vec![status.into()];
        values.extend(ids.iter().map(|id| js_num(*id)));
        values.push(js_num(viewer.id));
        db(env)?
            .prepare(format!(
                "UPDATE inbox SET status = ? WHERE id IN ({}) AND receiver_id = ?",
                placeholders(ids.len())
            ))
            .bind(&values)?
            .run()
            .await?;
    }

    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

pub(crate) async fn delete_inbox_item(
    env: &Env,
    viewer: &Viewer,
    item_id: &str,
) -> std::result::Result<Response, AppError> {
    let id = item_id
        .parse::<i64>()
        .map_err(|_| AppError::new(400, "Invalid inbox ID"))?;
    db(env)?
        .prepare("DELETE FROM inbox WHERE id = ? AND receiver_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

pub(crate) fn public_inbox_item(row: DbInboxRow) -> Value {
    let sender = row.sender_id
        .map(|id| json!({ "id": id, "username": row.sender_username, "nickname": row.sender_nickname }))
        .unwrap_or(Value::Null);
    json!({
        "id": row.id,
        "createdTs": row.created_ts,
        "sender": sender,
        "status": row.status,
        "message": safe_inbox_message(&row.message)
    })
}

pub(crate) async fn record_comment_inbox(
    env: &Env,
    sender_id: i64,
    receiver_id: i64,
    parent_uid: &str,
    comment_uid: &str,
) -> std::result::Result<(), AppError> {
    db(env)?.prepare("INSERT INTO inbox (created_ts, sender_id, receiver_id, status, message) VALUES (?, ?, ?, 'UNREAD', ?)")
        .bind(&[
            js_num(unix_now()),
            js_num(sender_id),
            js_num(receiver_id),
            comment_inbox_message(parent_uid, comment_uid).to_string().into(),
        ])?
        .run()
        .await?;
    Ok(())
}

pub(crate) fn comment_inbox_message(parent_uid: &str, comment_uid: &str) -> Value {
    json!({
        "type": "memo.comment.created",
        "memoUid": parent_uid,
        "commentUid": comment_uid
    })
}

pub(crate) fn safe_inbox_message(value: &str) -> Value {
    serde_json::from_str::<Value>(value).unwrap_or_else(|_| json!({ "type": "unknown" }))
}
