use super::*;

pub(crate) async fn list_webhooks(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let rows = db(env)?
        .prepare("SELECT * FROM webhook WHERE creator_id = ? ORDER BY created_ts DESC")
        .bind(&[js_num(viewer.id)])?
        .all()
        .await?;
    let webhooks: Vec<DbWebhook> = rows.results()?;
    let payload: Vec<Value> = webhooks.into_iter().map(public_webhook).collect();
    json_response(json!({ "webhooks": payload }), 200).map_err(AppError::from)
}

pub(crate) async fn create_webhook(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let url = normalize_http_url(
        body.get("url").and_then(Value::as_str).unwrap_or(""),
        "Invalid webhook URL",
    )?;
    if name.is_empty() {
        return Err(AppError::new(400, "Name is required"));
    }
    let now = unix_now();
    db(env)?.prepare("INSERT INTO webhook (created_ts, updated_ts, creator_id, name, url) VALUES (?, ?, ?, ?, ?)")
        .bind(&[js_num(now), js_num(now), js_num(viewer.id), name.into(), url.clone().into()])?
        .run()
        .await?;
    let id: Option<i64> = db(env)?.prepare("SELECT id FROM webhook WHERE creator_id = ? AND name = ? AND url = ? ORDER BY id DESC LIMIT 1")
        .bind(&[js_num(viewer.id), name.into(), url.clone().into()])?
        .first(Some("id"))
        .await?;
    let id = id.unwrap_or(0);
    record_audit(
        env,
        Some(viewer),
        "webhook.create",
        &id.to_string(),
        json!({ "name": name }),
    )
    .await;
    json_response(json!({ "webhook": { "id": id, "name": name, "url": url, "rowStatus": "NORMAL", "createdTs": now, "updatedTs": now } }), 201).map_err(AppError::from)
}

pub(crate) async fn update_webhook(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    webhook_id: &str,
) -> std::result::Result<Response, AppError> {
    let id = webhook_id
        .parse::<i64>()
        .map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let existing: Option<DbWebhook> = db(env)?
        .prepare("SELECT * FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let existing = existing.ok_or_else(|| AppError::new(404, "Webhook not found"))?;
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&existing.name);
    let url = if let Some(value) = body.get("url").and_then(Value::as_str) {
        normalize_http_url(value, "Invalid webhook URL")?
    } else {
        existing.url
    };
    let row_status = body
        .get("rowStatus")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "NORMAL" | "ARCHIVED"))
        .unwrap_or(&existing.row_status);
    let now = unix_now();
    db(env)?
        .prepare(
            "UPDATE webhook SET name = ?, url = ?, row_status = ?, updated_ts = ? WHERE id = ?",
        )
        .bind(&[
            name.into(),
            url.clone().into(),
            row_status.into(),
            js_num(now),
            js_num(id),
        ])?
        .run()
        .await?;
    json_response(json!({ "webhook": { "id": id, "name": name, "url": url, "rowStatus": row_status, "updatedTs": now } }), 200).map_err(AppError::from)
}

pub(crate) async fn delete_webhook(
    env: &Env,
    viewer: &Viewer,
    webhook_id: &str,
) -> std::result::Result<Response, AppError> {
    let id = webhook_id
        .parse::<i64>()
        .map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let existing: Option<i64> = db(env)?
        .prepare("SELECT id FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(Some("id"))
        .await?;
    if existing.is_none() {
        return Err(AppError::new(404, "Webhook not found"));
    }
    db(env)?
        .prepare("DELETE FROM webhook WHERE id = ?")
        .bind(&[js_num(id)])?
        .run()
        .await?;
    record_audit(env, Some(viewer), "webhook.delete", webhook_id, json!({})).await;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

fn public_webhook(webhook: DbWebhook) -> Value {
    json!({
        "id": webhook.id,
        "name": webhook.name,
        "url": webhook.url,
        "rowStatus": webhook.row_status,
        "createdTs": webhook.created_ts,
        "updatedTs": webhook.updated_ts
    })
}
