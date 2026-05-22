async fn create_share(req: &mut Request, env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let expires_ts = body.get("expiresTs").and_then(Value::as_i64);
    let share_uid = generate_uid("s");
    let now = unix_now();
    db(env)?.prepare("INSERT INTO memo_share (uid, memo_id, creator_id, created_ts, expires_ts) VALUES (?, ?, ?, ?, ?)")
        .bind(&[
            share_uid.clone().into(),
            js_num(memo.id),
            js_num(viewer.id),
            js_num(now),
            expires_ts.map(js_num).unwrap_or(JsValue::NULL),
        ])?
        .run()
        .await?;
    emit_memo_change(env, "share.created", &memo, json!({ "shareUid": share_uid.clone(), "expiresTs": expires_ts })).await;
    json_response(json!({
        "share": {
            "uid": share_uid,
            "memoUid": memo.uid,
            "createdTs": now,
            "expiresTs": expires_ts,
            "url": format!("/api/v1/shares/{}", share_uid)
        }
    }), 201).map_err(AppError::from)
}

async fn list_shares(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let rows = db(env)?.prepare("SELECT * FROM memo_share WHERE memo_id = ? ORDER BY created_ts DESC")
        .bind(&[js_num(memo.id)])?
        .all()
        .await?;
    let shares: Vec<DbShare> = rows.results()?;
    let payload: Vec<Value> = shares.into_iter().map(|share| json!({
        "id": share.id,
        "uid": share.uid,
        "memoUid": memo.uid,
        "createdTs": share.created_ts,
        "expiresTs": share.expires_ts,
        "url": format!("/api/v1/shares/{}", share.uid)
    })).collect();
    json_response(json!({ "shares": payload }), 200).map_err(AppError::from)
}

async fn delete_share(env: &Env, viewer: &Viewer, uid: &str, share_id: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    let id = share_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid share ID"))?;
    let share: Option<DbShare> = db(env)?.prepare("SELECT * FROM memo_share WHERE id = ? AND memo_id = ?")
        .bind(&[js_num(id), js_num(memo.id)])?
        .first(None)
        .await?;
    let share = share.ok_or_else(|| AppError::new(404, "Share not found"))?;
    if viewer.role != "ADMIN" && share.creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    db(env)?.prepare("DELETE FROM memo_share WHERE id = ?")
        .bind(&[js_num(id)])?
        .run()
        .await?;
    emit_memo_change(env, "share.deleted", &memo, json!({ "shareId": share.id, "shareUid": share.uid })).await;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn get_ai_settings(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let settings = resolve_ai_settings(env).await?;
    json_response(json!({ "settings": public_ai_settings(&settings) }), 200).map_err(AppError::from)
}

async fn update_ai_settings(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let previous = read_stored_ai_settings(env).await?;
    let next = merge_ai_settings(&previous, &body)?;
    db(env)?.prepare("INSERT INTO system_setting (name, value, description) VALUES ('ai.settings', ?, 'AI model settings') ON CONFLICT(name) DO UPDATE SET value = excluded.value")
        .bind(&[serde_json::to_string(&next).map_err(|error| AppError::new(500, error.to_string()))?.into()])?
        .run()
        .await?;
    json_response(json!({ "settings": public_ai_settings(&next) }), 200).map_err(AppError::from)
}

async fn test_ai_settings(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let base = resolve_ai_settings(env).await?;
    let settings = merge_ai_settings(&base, &body)?;
    if settings.api_key.trim().is_empty() {
        return Err(AppError::new(400, "AI API Key is required"));
    }
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", settings.api_key))?;
    headers.set("Content-Type", "application/json")?;
    let payload = json!({
        "model": settings.model,
        "temperature": 0,
        "messages": [
            { "role": "system", "content": "Return ok." },
            { "role": "user", "content": "ping" }
        ],
        "max_tokens": 8
    });
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&payload.to_string())));
    let request = Request::new_with_init(&format!("{}/chat/completions", settings.base_url.trim_end_matches('/')), &init)?;
    let response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(502, format!("AI API returned HTTP {}", response.status_code())));
    }
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn resolve_ai_settings(env: &Env) -> std::result::Result<AiSettings, AppError> {
    let stored = read_stored_ai_settings(env).await?;
    Ok(AiSettings {
        base_url: normalize_http_url(
            if stored.base_url.is_empty() {
                env.var("AI_BASE_URL").map(|value| value.to_string()).unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
            } else {
                stored.base_url
            },
            "Invalid AI Base URL",
        )?,
        model: if stored.model.is_empty() {
            env.var("AI_MODEL").map(|value| value.to_string()).unwrap_or_else(|_| "gpt-4o-mini".to_string())
        } else {
            stored.model
        },
        api_key: if stored.api_key.is_empty() {
            env.secret("AI_API_KEY").map(|value| value.to_string()).unwrap_or_default()
        } else {
            stored.api_key
        },
    })
}

async fn read_stored_ai_settings(env: &Env) -> std::result::Result<AiSettings, AppError> {
    let value: Option<String> = db(env)?.prepare("SELECT value FROM system_setting WHERE name = 'ai.settings'")
        .first(Some("value"))
        .await?;
    let stored = value.and_then(|text| serde_json::from_str::<AiSettings>(&text).ok());
    Ok(stored.unwrap_or(AiSettings {
        base_url: "https://api.openai.com/v1".to_string(),
        model: "gpt-4o-mini".to_string(),
        api_key: String::new(),
    }))
}

fn merge_ai_settings(previous: &AiSettings, update: &Value) -> std::result::Result<AiSettings, AppError> {
    let base_url = update.get("baseUrl").and_then(Value::as_str).unwrap_or(&previous.base_url);
    let model = update.get("model").and_then(Value::as_str).unwrap_or(&previous.model).trim();
    let api_key = update.get("apiKey").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).unwrap_or(&previous.api_key);
    Ok(AiSettings {
        base_url: normalize_http_url(base_url, "Invalid AI Base URL")?,
        model: if model.is_empty() { "gpt-4o-mini".to_string() } else { model.to_string() },
        api_key: api_key.to_string(),
    })
}

fn public_ai_settings(settings: &AiSettings) -> Value {
    json!({
        "baseUrl": settings.base_url,
        "model": settings.model,
        "configured": !settings.api_key.trim().is_empty()
    })
}

async fn create_backup(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let artifact = create_backup_artifact(env).await?;
    record_audit(env, Some(viewer), "backup.create", &artifact.key, json!({ "size": artifact.size })).await;
    json_response(backup_artifact_payload(&artifact), 201).map_err(AppError::from)
}

async fn create_scheduled_backup(env: &Env) -> std::result::Result<BackupArtifact, AppError> {
    let artifact = create_backup_artifact(env).await?;
    record_audit(env, None, "backup.create", &artifact.key, json!({ "size": artifact.size, "source": "scheduled" })).await;
    Ok(artifact)
}

async fn create_backup_artifact(env: &Env) -> std::result::Result<BackupArtifact, AppError> {
    let key = backup_key();
    let body = serde_json::to_string_pretty(&build_backup_payload(env).await?).map_err(|error| AppError::new(500, error.to_string()))?;
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

fn backup_artifact_payload(artifact: &BackupArtifact) -> Value {
    json!({ "backup": { "key": artifact.key, "size": artifact.size } })
}

async fn list_backups(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let listed = env.bucket("MEMOS_BUCKET")?.list().prefix("backups/").execute().await?;
    let mut backups: Vec<Value> = listed.objects().into_iter().map(|object| json!({
        "key": object.key(),
        "size": object.size(),
        "uploaded": object.uploaded().to_string()
    })).collect();
    backups.sort_by(|a, b| b.get("uploaded").and_then(Value::as_str).cmp(&a.get("uploaded").and_then(Value::as_str)));
    json_response(json!({ "backups": backups }), 200).map_err(AppError::from)
}

async fn download_backup(env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let key = url.query_pairs().find(|(name, _)| name == "key").map(|(_, value)| value.to_string()).unwrap_or_default();
    if !key.starts_with("backups/") {
        return Err(AppError::new(400, "Invalid backup key"));
    }
    let object = env.bucket("MEMOS_BUCKET")?.get(key.clone()).execute().await?
        .ok_or_else(|| AppError::new(404, "Backup not found"))?;
    let body = object.body().ok_or_else(|| AppError::new(404, "Backup not found"))?.response_body()?;
    let filename = key.rsplit('/').next().unwrap_or("memos-backup.json");
    let mut response = ResponseBuilder::new().body(body);
    response.headers_mut().set("Content-Type", "application/json")?;
    response.headers_mut().set("Content-Disposition", &format!("attachment; filename=\"{}\"", filename))?;
    Ok(response)
}

async fn preview_backup(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let payload = read_backup_payload(req, env).await?;
    json_response(json!({ "preview": backup_preview(&payload) }), 200).map_err(AppError::from)
}

async fn restore_backup(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
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
    record_audit(env, Some(viewer), "backup.restore", "backup", preview.clone()).await;
    json_response(json!({ "restored": preview }), 200).map_err(AppError::from)
}

async fn read_backup_payload(req: &mut Request, env: &Env) -> std::result::Result<Value, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    if let Some(payload) = body.get("payload") {
        return Ok(payload.clone());
    }
    let key = body.get("key").and_then(Value::as_str).unwrap_or("");
    if !key.starts_with("backups/") {
        return Err(AppError::new(400, "Invalid backup key"));
    }
    let object = env.bucket("MEMOS_BUCKET")?.get(key.to_string()).execute().await?
        .ok_or_else(|| AppError::new(404, "Backup not found"))?;
    let text = object.body().ok_or_else(|| AppError::new(404, "Backup not found"))?.text().await?;
    serde_json::from_str(&text).map_err(|_| AppError::new(400, "Invalid backup payload"))
}

async fn build_backup_payload(env: &Env) -> std::result::Result<Value, AppError> {
    let users = db(env)?.prepare("SELECT id, created_ts, updated_ts, row_status, username, role, email, nickname, avatar_url, description FROM \"user\" ORDER BY id").all().await?.results::<Value>()?;
    let memos = db(env)?.prepare("SELECT * FROM memo ORDER BY created_ts, id").all().await?.results::<Value>()?;
    let attachments = db(env)?.prepare("SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload FROM attachment ORDER BY created_ts, id").all().await?.results::<Value>()?;
    let relations = db(env)?.prepare("SELECT * FROM memo_relation ORDER BY memo_id, related_memo_id").all().await?.results::<Value>()?;
    Ok(json!({
        "exportedAt": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default(),
        "users": users,
        "memos": memos,
        "attachments": attachments,
        "relations": relations
    }))
}

fn backup_preview(payload: &Value) -> Value {
    json!({
        "userCount": payload.get("users").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "memoCount": payload.get("memos").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "attachmentCount": payload.get("attachments").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "relationCount": payload.get("relations").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
    })
}

fn backup_key() -> String {
    let stamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_else(|| unix_now().to_string()).replace([':', '.'], "-");
    format!("backups/memos-{}.json", stamp)
}

async fn list_audit_logs(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    ensure_audit_log_table(env).await?;
    let rows = db(env)?.prepare("SELECT audit_log.*, \"user\".username AS actor_username FROM audit_log LEFT JOIN \"user\" ON \"user\".id = audit_log.actor_id ORDER BY audit_log.created_ts DESC, audit_log.id DESC LIMIT 100")
        .all()
        .await?;
    let logs: Vec<DbAuditLog> = rows.results()?;
    let payload: Vec<Value> = logs.into_iter().map(|row| json!({
        "id": row.id,
        "createdTs": row.created_ts,
        "actorId": row.actor_id,
        "actorUsername": row.actor_username,
        "action": row.action,
        "actionLabel": audit_action_label(&row.action),
        "target": row.target,
        "detail": serde_json::from_str::<Value>(&row.detail).unwrap_or_else(|_| json!({}))
    })).collect();
    json_response(json!({ "logs": payload }), 200).map_err(AppError::from)
}

async fn list_webhooks(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let rows = db(env)?.prepare("SELECT * FROM webhook WHERE creator_id = ? ORDER BY created_ts DESC")
        .bind(&[js_num(viewer.id)])?
        .all()
        .await?;
    let webhooks: Vec<DbWebhook> = rows.results()?;
    let payload: Vec<Value> = webhooks.into_iter().map(public_webhook).collect();
    json_response(json!({ "webhooks": payload }), 200).map_err(AppError::from)
}

async fn create_webhook(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let name = body.get("name").and_then(Value::as_str).unwrap_or("").trim();
    let url = normalize_http_url(body.get("url").and_then(Value::as_str).unwrap_or(""), "Invalid webhook URL")?;
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
    record_audit(env, Some(viewer), "webhook.create", &id.to_string(), json!({ "name": name })).await;
    json_response(json!({ "webhook": { "id": id, "name": name, "url": url, "rowStatus": "NORMAL", "createdTs": now, "updatedTs": now } }), 201).map_err(AppError::from)
}

async fn update_webhook(req: &mut Request, env: &Env, viewer: &Viewer, webhook_id: &str) -> std::result::Result<Response, AppError> {
    let id = webhook_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let existing: Option<DbWebhook> = db(env)?.prepare("SELECT * FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let existing = existing.ok_or_else(|| AppError::new(404, "Webhook not found"))?;
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let name = body.get("name").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).unwrap_or(&existing.name);
    let url = if let Some(value) = body.get("url").and_then(Value::as_str) {
        normalize_http_url(value, "Invalid webhook URL")?
    } else {
        existing.url
    };
    let row_status = body.get("rowStatus").and_then(Value::as_str).filter(|value| matches!(*value, "NORMAL" | "ARCHIVED")).unwrap_or(&existing.row_status);
    let now = unix_now();
    db(env)?.prepare("UPDATE webhook SET name = ?, url = ?, row_status = ?, updated_ts = ? WHERE id = ?")
        .bind(&[name.into(), url.clone().into(), row_status.into(), js_num(now), js_num(id)])?
        .run()
        .await?;
    json_response(json!({ "webhook": { "id": id, "name": name, "url": url, "rowStatus": row_status, "updatedTs": now } }), 200).map_err(AppError::from)
}

async fn delete_webhook(env: &Env, viewer: &Viewer, webhook_id: &str) -> std::result::Result<Response, AppError> {
    let id = webhook_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let existing: Option<i64> = db(env)?.prepare("SELECT id FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(Some("id"))
        .await?;
    if existing.is_none() {
        return Err(AppError::new(404, "Webhook not found"));
    }
    db(env)?.prepare("DELETE FROM webhook WHERE id = ?").bind(&[js_num(id)])?.run().await?;
    record_audit(env, Some(viewer), "webhook.delete", webhook_id, json!({})).await;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn list_webhook_deliveries(env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let webhook_id = url.query_pairs().find(|(key, _)| key == "webhookId").and_then(|(_, value)| value.parse::<i64>().ok());
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
    let payload: Vec<Value> = deliveries.into_iter().map(public_webhook_delivery).collect();
    json_response(json!({ "deliveries": payload }), 200).map_err(AppError::from)
}

async fn test_webhook(env: &Env, viewer: &Viewer, webhook_id: &str) -> std::result::Result<Response, AppError> {
    let id = webhook_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid webhook ID"))?;
    let webhook: Option<DbWebhook> = db(env)?.prepare("SELECT * FROM webhook WHERE id = ? AND creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let webhook = webhook.ok_or_else(|| AppError::new(404, "Webhook not found"))?;
    let delivery = send_and_record_webhook(env, webhook.id, viewer.id, &webhook.url, "webhook.test", &json!({
        "event": "webhook.test",
        "timestamp": unix_now(),
        "payload": { "ok": true, "source": "memos-worker" }
    }).to_string()).await?;
    json_response(json!({ "delivery": delivery.map(public_webhook_delivery) }), 201).map_err(AppError::from)
}

async fn retry_webhook_delivery(env: &Env, viewer: &Viewer, delivery_id: &str) -> std::result::Result<Response, AppError> {
    let id = delivery_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid delivery ID"))?;
    let row: Option<DbWebhookDelivery> = db(env)?.prepare("SELECT webhook_delivery.*, webhook.name AS webhook_name, webhook.url AS webhook_url FROM webhook_delivery JOIN webhook ON webhook.id = webhook_delivery.webhook_id WHERE webhook_delivery.id = ? AND webhook_delivery.creator_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .first(None)
        .await?;
    let row = row.ok_or_else(|| AppError::new(404, "Webhook delivery not found"))?;
    let url = row.webhook_url.as_deref().unwrap_or("");
    let delivery = send_and_record_webhook(env, row.webhook_id, viewer.id, url, &row.event, &row.request_body).await?;
    json_response(json!({ "delivery": delivery.map(public_webhook_delivery) }), 200).map_err(AppError::from)
}

async fn send_and_record_webhook(env: &Env, webhook_id: i64, creator_id: i64, url: &str, event: &str, request_body: &str) -> std::result::Result<Option<DbWebhookDelivery>, AppError> {
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
    let status = if status_code.map(|code| (200..300).contains(&code)).unwrap_or(false) { "SUCCESS" } else { "FAILED" };
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

async fn list_inbox(env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let rows = db(env)?.prepare("SELECT inbox.*, \"user\".username AS sender_username, \"user\".nickname AS sender_nickname FROM inbox LEFT JOIN \"user\" ON \"user\".id = inbox.sender_id WHERE inbox.receiver_id = ? ORDER BY inbox.created_ts DESC LIMIT 100")
        .bind(&[js_num(viewer.id)])?
        .all()
        .await?;
    let inbox: Vec<DbInboxRow> = rows.results()?;
    let unread_count: Option<i64> = db(env)?.prepare("SELECT COUNT(*) AS count FROM inbox WHERE receiver_id = ? AND status = 'UNREAD'")
        .bind(&[js_num(viewer.id)])?
        .first(Some("count"))
        .await?;
    let payload: Vec<Value> = inbox.into_iter().map(public_inbox_item).collect();
    json_response(json!({ "inbox": payload, "unreadCount": unread_count.unwrap_or(0) }), 200).map_err(AppError::from)
}

async fn update_inbox_status(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let status = if body.get("status").and_then(Value::as_str) == Some("READ") { "READ" } else { "UNREAD" };
    let ids: Vec<i64> = body.get("ids")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_i64).filter(|id| *id > 0).collect())
        .unwrap_or_default();

    if ids.is_empty() {
        db(env)?.prepare("UPDATE inbox SET status = ? WHERE receiver_id = ?")
            .bind(&[status.into(), js_num(viewer.id)])?
            .run()
            .await?;
    } else {
        let mut values: Vec<JsValue> = vec![status.into()];
        values.extend(ids.iter().map(|id| js_num(*id)));
        values.push(js_num(viewer.id));
        db(env)?.prepare(format!("UPDATE inbox SET status = ? WHERE id IN ({}) AND receiver_id = ?", placeholders(ids.len())))
            .bind(&values)?
            .run()
            .await?;
    }

    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn delete_inbox_item(env: &Env, viewer: &Viewer, item_id: &str) -> std::result::Result<Response, AppError> {
    let id = item_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid inbox ID"))?;
    db(env)?.prepare("DELETE FROM inbox WHERE id = ? AND receiver_id = ?")
        .bind(&[js_num(id), js_num(viewer.id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

fn public_inbox_item(row: DbInboxRow) -> Value {
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

async fn record_comment_inbox(env: &Env, sender_id: i64, receiver_id: i64, parent_uid: &str, comment_uid: &str) -> std::result::Result<(), AppError> {
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

fn comment_inbox_message(parent_uid: &str, comment_uid: &str) -> Value {
    json!({
        "type": "memo.comment.created",
        "memoUid": parent_uid,
        "commentUid": comment_uid
    })
}

fn safe_inbox_message(value: &str) -> Value {
    serde_json::from_str::<Value>(value).unwrap_or_else(|_| json!({ "type": "unknown" }))
}
