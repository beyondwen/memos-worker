use super::*;

pub(crate) async fn list_webhook_deliveries(
    env: &Env,
    url: &Url,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let webhook_id = url
        .query_pairs()
        .find(|(key, _)| key == "webhookId")
        .and_then(|(_, value)| value.parse::<i64>().ok());
    let rows = if let Some(id) = webhook_id.filter(|id| *id > 0) {
        db(env)?.prepare("SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url FROM webhook_delivery JOIN webhook ON webhook.id = webhook_delivery.webhook_id WHERE webhook_delivery.creator_id = ? AND webhook_delivery.webhook_id = ? ORDER BY webhook_delivery.created_ts DESC, webhook_delivery.id DESC LIMIT 50")
            .bind(&[js_num(viewer.id), js_num(id)])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url FROM webhook_delivery JOIN webhook ON webhook.id = webhook_delivery.webhook_id WHERE webhook_delivery.creator_id = ? ORDER BY webhook_delivery.created_ts DESC, webhook_delivery.id DESC LIMIT 50")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    };
    let deliveries: Vec<DbWebhookDelivery> = rows.results()?;
    let payload: Vec<Value> = deliveries
        .into_iter()
        .map(public_webhook_delivery)
        .collect();
    json_response(json!({ "deliveries": payload }), 200).map_err(AppError::from)
}

pub(crate) async fn test_webhook(
    env: &Env,
    viewer: &Viewer,
    webhook_id: &str,
) -> std::result::Result<Response, AppError> {
    let id = webhook_id
        .parse::<i64>()
        .map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let webhook: Option<DbWebhook> = db(env)?
        .prepare("SELECT * FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let webhook = webhook.ok_or_else(|| AppError::new(404, "Webhook not found"))?;
    let delivery = send_and_record_webhook(
        env,
        webhook.id,
        viewer.id,
        &webhook.url,
        "webhook.test",
        &json!({
            "event": "webhook.test",
            "timestamp": unix_now(),
            "payload": { "ok": true, "source": "memos-worker" }
        })
        .to_string(),
    )
    .await?;
    json_response(
        json!({ "delivery": delivery.map(public_webhook_delivery) }),
        201,
    )
    .map_err(AppError::from)
}

pub(crate) async fn retry_webhook_delivery(
    env: &Env,
    viewer: &Viewer,
    delivery_id: &str,
) -> std::result::Result<Response, AppError> {
    let id = delivery_id
        .parse::<i64>()
        .map_err(|_| AppError::new(400, "Invalid delivery ID"))?;
    let row: Option<DbWebhookDelivery> = db(env)?.prepare("SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url FROM webhook_delivery JOIN webhook ON webhook.id = webhook_delivery.webhook_id WHERE webhook_delivery.id = ? AND webhook_delivery.creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let row = row.ok_or_else(|| AppError::new(404, "Webhook delivery not found"))?;
    let url = row.webhook_url.as_deref().unwrap_or("");
    let delivery = send_and_record_webhook(
        env,
        row.webhook_id,
        viewer.id,
        url,
        &row.event,
        &row.request_body,
    )
    .await?;
    json_response(
        json!({ "delivery": delivery.map(public_webhook_delivery) }),
        200,
    )
    .map_err(AppError::from)
}

pub(crate) async fn send_and_record_webhook(
    env: &Env,
    webhook_id: i64,
    creator_id: i64,
    url: &str,
    event: &str,
    request_body: &str,
) -> std::result::Result<Option<DbWebhookDelivery>, AppError> {
    let started = js_sys::Date::now();
    let mut status_code: Option<i64> = None;
    let mut response_body = String::new();
    let mut error = String::new();
    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(request_body)));
    match Request::new_with_init(url, &init) {
        Ok(request) => match Fetch::Request(request).send().await {
            Ok(mut response) => {
                let code = response.status_code() as i64;
                status_code = Some(code);
                response_body = response.text().await.unwrap_or_default();
                if !(200..300).contains(&code) {
                    error = format!("HTTP {}", code);
                }
            }
            Err(err) => error = err.to_string(),
        },
        Err(err) => error = err.to_string(),
    }
    let duration_ms = (js_sys::Date::now() - started).max(0.0).round() as i64;
    let status = if status_code
        .map(|code| (200..300).contains(&code))
        .unwrap_or(false)
    {
        "SUCCESS"
    } else {
        "FAILED"
    };
    db(env)?.prepare("INSERT INTO webhook_delivery (webhook_id, creator_id, created_ts, event, status, status_code, duration_ms, error, request_body, response_body) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&[
            js_num(webhook_id),
            js_num(creator_id),
            js_num(unix_now()),
            truncate(event, 200).into(),
            status.into(),
            status_code.map(js_num).unwrap_or(JsValue::NULL),
            js_num(duration_ms),
            truncate(&error, 1000).into(),
            truncate(request_body, 12000).into(),
            truncate(&response_body, 4000).into(),
        ])?
        .run()
        .await?;
    let inserted: Option<DbWebhookDelivery> = db(env)?.prepare("SELECT * FROM webhook_delivery WHERE webhook_id = ? AND creator_id = ? ORDER BY id DESC LIMIT 1")
        .bind(&[js_num(webhook_id), js_num(creator_id)])?
        .first(None)
        .await?;
    prune_webhook_deliveries(env, creator_id).await;
    Ok(inserted)
}

async fn prune_webhook_deliveries(env: &Env, creator_id: i64) {
    if let Ok(database) = db(env) {
        let stmt = database.prepare("DELETE FROM webhook_delivery WHERE creator_id = ? AND id NOT IN (SELECT id FROM webhook_delivery WHERE creator_id = ? ORDER BY created_ts DESC, id DESC LIMIT 200)");
        if let Ok(bound) = stmt.bind(&[js_num(creator_id), js_num(creator_id)]) {
            let _ = bound.run().await;
        }
    }
}

fn public_webhook_delivery(delivery: DbWebhookDelivery) -> Value {
    json!({
        "id": delivery.id,
        "webhookId": delivery.webhook_id,
        "webhookName": delivery.webhook_name.unwrap_or_default(),
        "webhookUrl": delivery.webhook_url.unwrap_or_default(),
        "createdTs": delivery.created_ts,
        "event": delivery.event,
        "status": delivery.status,
        "statusCode": delivery.status_code,
        "durationMs": delivery.duration_ms,
        "error": delivery.error,
        "responseBody": delivery.response_body
    })
}
