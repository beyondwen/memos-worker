use super::*;

pub(crate) async fn emit_memo_event(env: &Env, event_type: &str, memo: &DbMemo) {
    emit_memo_change(env, event_type, memo, json!({})).await;
}

pub(crate) async fn emit_memo_change(env: &Env, event_type: &str, memo: &DbMemo, detail: Value) {
    if let Err(error) = record_memo_event_with_detail(env, event_type, memo, detail.clone()).await {
        console_log!("memo event record failed: {}", error.message);
    }
    if let Err(error) = fire_memo_webhooks(env, event_type, memo, detail).await {
        console_log!("memo webhook dispatch failed: {}", error.message);
    }
}

pub(crate) async fn record_memo_event_with_detail(
    env: &Env,
    event_type: &str,
    memo: &DbMemo,
    detail: Value,
) -> std::result::Result<(), AppError> {
    ensure_memo_event_table(env).await?;
    let payload = memo_event_payload_with_detail(event_type, memo, detail);
    db(env)?.prepare("INSERT INTO memo_event (created_ts, event_type, name, visibility, creator_id, payload) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&[
            js_num(unix_now()),
            event_type.into(),
            format!("memos/{}", memo.uid).into(),
            memo.visibility.clone().into(),
            js_num(memo.creator_id),
            payload.to_string().into(),
        ])?
        .run()
        .await?;
    Ok(())
}

pub(crate) async fn emit_bulk_memo_events(
    env: &Env,
    memos: &[DbMemo],
    action: &str,
    updated: usize,
    deleted: usize,
    skipped: usize,
    updated_ts: i64,
    row_status: Option<&str>,
    visibility: Option<&str>,
) {
    let detail = json!({
        "action": action,
        "updated": updated,
        "deleted": deleted,
        "skipped": skipped
    });
    for memo in memos {
        let event_memo = DbMemo {
            updated_ts,
            row_status: row_status.unwrap_or(&memo.row_status).to_string(),
            visibility: visibility.unwrap_or(&memo.visibility).to_string(),
            ..memo.clone()
        };
        emit_memo_change(env, "memo.bulk.updated", &event_memo, detail.clone()).await;
    }
}

pub(crate) fn memo_event_payload(event_type: &str, memo: &DbMemo) -> Value {
    json!({
        "type": event_type,
        "name": format!("memos/{}", memo.uid),
        "visibility": memo.visibility,
        "creatorId": memo.creator_id
    })
}

pub(crate) fn memo_event_payload_with_detail(
    event_type: &str,
    memo: &DbMemo,
    detail: Value,
) -> Value {
    let mut payload = memo_event_payload(event_type, memo);
    if let Value::Object(base) = &mut payload {
        match detail {
            Value::Object(extra) => {
                for (key, value) in extra {
                    base.insert(key, value);
                }
            }
            Value::Null => {}
            other => {
                base.insert("detail".to_string(), other);
            }
        }
    }
    payload
}

pub(crate) fn memo_webhook_body(
    event_type: &str,
    memo: &DbMemo,
    timestamp: i64,
    detail: Value,
) -> Value {
    json!({
        "event": event_type,
        "timestamp": timestamp,
        "payload": {
            "memo": public_memo(memo.clone()),
            "detail": detail
        }
    })
}

pub(crate) async fn fire_memo_webhooks(
    env: &Env,
    event_type: &str,
    memo: &DbMemo,
    detail: Value,
) -> std::result::Result<(), AppError> {
    let rows = db(env)?
        .prepare("SELECT * FROM webhook WHERE creator_id = ? AND row_status = 'NORMAL' ORDER BY id")
        .bind(&[js_num(memo.creator_id)])?
        .all()
        .await?;
    let webhooks: Vec<DbWebhook> = rows.results()?;
    if webhooks.is_empty() {
        return Ok(());
    }
    let body = memo_webhook_body(event_type, memo, unix_now(), detail).to_string();
    for webhook in webhooks {
        if let Err(error) = send_and_record_webhook(
            env,
            webhook.id,
            memo.creator_id,
            &webhook.url,
            event_type,
            &body,
        )
        .await
        {
            console_log!("webhook delivery failed: {}", error.message);
        }
    }
    Ok(())
}

pub(crate) async fn prune_memo_events(
    env: &Env,
    retention_days: i64,
) -> std::result::Result<i64, AppError> {
    ensure_memo_event_table(env).await?;
    let cutoff = memo_event_retention_cutoff(unix_now(), retention_days);
    db(env)?
        .prepare("DELETE FROM memo_event WHERE created_ts < ?")
        .bind(&[js_num(cutoff)])?
        .run()
        .await?;
    let count: Option<i64> = db(env)?
        .prepare("SELECT changes() AS count")
        .first(Some("count"))
        .await?;
    Ok(count.unwrap_or(0))
}

pub(crate) fn memo_event_retention_cutoff(now: i64, retention_days: i64) -> i64 {
    now - retention_days.max(0) * 24 * 60 * 60
}

pub(crate) async fn ensure_memo_event_table(env: &Env) -> std::result::Result<(), AppError> {
    db(env)?.prepare("CREATE TABLE IF NOT EXISTS memo_event (id INTEGER PRIMARY KEY AUTOINCREMENT, created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), event_type TEXT NOT NULL, name TEXT NOT NULL, visibility TEXT NOT NULL DEFAULT 'PRIVATE', creator_id INTEGER NOT NULL, payload TEXT NOT NULL DEFAULT '{}')")
        .run()
        .await?;
    db(env)?
        .prepare("CREATE INDEX IF NOT EXISTS idx_memo_event_created ON memo_event(created_ts, id)")
        .run()
        .await?;
    db(env)?.prepare("CREATE INDEX IF NOT EXISTS idx_memo_event_visibility ON memo_event(visibility, creator_id, id)")
        .run()
        .await?;
    Ok(())
}
