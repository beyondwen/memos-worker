async fn migration_preview(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let (memos, truncated) = fetch_original_memos(&options).await?;
    let preview = summarize_original_memos(&memos, truncated);
    let _ = env;
    json_response(json!({ "preview": preview }), 200).map_err(AppError::from)
}

async fn migration_import(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let progress = import_original_memos(env, viewer, &options, None).await?;
    record_migration_audit(env, viewer, &options, &progress).await;
    json_response(json!({ "result": progress }), 200).map_err(AppError::from)
}

async fn migration_import_stream(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let env = env.clone();
    let viewer = viewer.clone();
    let (sender, receiver) = mpsc::unbounded::<Vec<u8>>();
    wasm_bindgen_futures::spawn_local(async move {
        let mut sender = sender;
        record_migration_start_audit(&env, &viewer, &options).await;
        let result = import_original_memos_streaming(&env, &viewer, &options, |event, progress| {
            send_sse_chunk(&mut sender, event, progress)
        }).await;
        match result {
            Ok(progress) => {
                record_migration_audit(&env, &viewer, &options, &progress).await;
                let _ = send_sse_chunk(&mut sender, "done", &progress);
            }
            Err(error) => {
                record_migration_error_audit(&env, &viewer, &options, &error.message).await;
                let _ = send_sse_chunk(&mut sender, "error", &json!({ "error": error.message }));
            }
        }
    });

    let stream = receiver.map(|chunk| Ok::<Vec<u8>, worker::Error>(chunk));
    let mut response = Response::from_stream(stream)?;
    response.headers_mut().set("Content-Type", "text/event-stream; charset=utf-8")?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    response.headers_mut().set("X-Accel-Buffering", "no")?;
    Ok(response)
}

async fn read_migration_request(req: &mut Request) -> std::result::Result<MigrationOptions, AppError> {
    let body: MigrationRequest = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let raw = body.base_url.unwrap_or_default();
    let mut base_url = Url::parse(raw.trim()).map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
    if base_url.scheme() != "http" && base_url.scheme() != "https" {
        return Err(AppError::new(400, "Only http and https URLs are supported"));
    }
    base_url.set_query(None);
    base_url.set_fragment(None);
    let mut base = base_url.to_string();
    while base.ends_with('/') {
        base.pop();
    }
    let access_token = body.access_token.unwrap_or_default().trim().to_string();
    if access_token.is_empty() {
        return Err(AppError::new(400, "Access token is required"));
    }
    Ok(MigrationOptions {
        base_url: base,
        access_token,
        include_archived: body.include_archived.unwrap_or(false),
    })
}

async fn fetch_original_memos(options: &MigrationOptions) -> std::result::Result<(Vec<OriginalMemo>, bool), AppError> {
    let mut all = Vec::new();
    let mut truncated = false;
    let states = if options.include_archived { vec!["NORMAL", "ARCHIVED"] } else { vec!["NORMAL"] };
    for state in states {
        let mut page_token = String::new();
        loop {
            let mut url = Url::parse(&format!("{}/api/v1/memos", options.base_url)).map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
            url.query_pairs_mut()
                .append_pair("pageSize", &MIGRATION_PAGE_SIZE.to_string())
                .append_pair("state", state);
            if !page_token.is_empty() {
                url.query_pairs_mut().append_pair("pageToken", &page_token);
            }
            let (memos, next_page_token) = fetch_original_memos_page(options, url.as_str()).await?;
            for memo in memos {
                if all.len() >= MIGRATION_MAX_MEMOS {
                    truncated = true;
                    break;
                }
                all.push(memo);
            }
            if truncated || next_page_token.is_empty() {
                break;
            }
            page_token = next_page_token;
        }
        if truncated {
            break;
        }
    }
    Ok((all, truncated))
}

async fn fetch_original_memos_page(options: &MigrationOptions, url: &str) -> std::result::Result<(Vec<OriginalMemo>, String), AppError> {
    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", options.access_token))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(url, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(400, format!("Original Memos API returned HTTP {}", response.status_code())));
    }
    let data: Value = response.json().await?;
    let memos = data.get("memos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value::<OriginalMemo>(value).ok())
        .collect();
    let next = data.get("nextPageToken").and_then(Value::as_str).unwrap_or("").to_string();
    Ok((memos, next))
}

async fn import_original_memos(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    mut events: Option<&mut Vec<String>>,
) -> std::result::Result<MigrationProgress, AppError> {
    let (memos, truncated) = fetch_original_memos(options).await?;
    let summary = summarize_original_memos(&memos, truncated);
    let mut progress = MigrationProgress {
        phase: "importing".to_string(),
        processed: 0,
        imported: 0,
        skipped: 0,
        memo_count: summary.memo_count,
        attachment_count: summary.attachment_count,
        relation_count: summary.relation_count,
        archived_count: summary.archived_count,
        truncated,
        state: None,
    };
    if let Some(buf) = events.as_deref_mut() {
        buf.push(sse_event("progress", &progress)?);
    }
    for memo in memos {
        if import_single_original_memo(env, viewer, &memo).await? {
            progress.imported += 1;
        } else {
            progress.skipped += 1;
        }
        progress.processed += 1;
        if let Some(buf) = events.as_deref_mut() {
            buf.push(sse_event("progress", &progress)?);
        }
    }
    progress.phase = "done".to_string();
    if let Some(buf) = events.as_deref_mut() {
        buf.push(sse_event("progress", &progress)?);
    }
    Ok(progress)
}

async fn import_original_memos_streaming<F>(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    mut on_progress: F,
) -> std::result::Result<MigrationProgress, AppError>
where
    F: FnMut(&str, &MigrationProgress) -> std::result::Result<(), AppError>,
{
    let states = if options.include_archived { vec!["NORMAL", "ARCHIVED"] } else { vec!["NORMAL"] };
    let mut progress = MigrationProgress {
        phase: "fetching".to_string(),
        processed: 0,
        imported: 0,
        skipped: 0,
        memo_count: 0,
        attachment_count: 0,
        relation_count: 0,
        archived_count: 0,
        truncated: false,
        state: None,
    };
    on_progress("progress", &progress)?;
    let mut imported_original_names = BTreeSet::new();

    for state in states {
        let mut page_token = String::new();
        loop {
            let previous_page_token = page_token.clone();
            progress.phase = "fetching".to_string();
            progress.state = Some(state.to_string());
            on_progress("progress", &progress)?;

            let mut url = Url::parse(&format!("{}/api/v1/memos", options.base_url)).map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
            url.query_pairs_mut()
                .append_pair("pageSize", &MIGRATION_PAGE_SIZE.to_string())
                .append_pair("state", state);
            if !page_token.is_empty() {
                url.query_pairs_mut().append_pair("pageToken", &page_token);
            }

            let (memos, next_page_token) = fetch_original_memos_page(options, url.as_str()).await?;
            let existing_names = existing_imported_original_names(env, viewer.id, &memos).await?;
            progress.phase = "importing".to_string();
            for memo in memos {
                if progress.memo_count >= MIGRATION_MAX_MEMOS {
                    progress.truncated = true;
                    break;
                }
                progress.memo_count += 1;
                progress.attachment_count += memo.attachments.as_ref().map(Vec::len).unwrap_or(0);
                progress.relation_count += memo.relations.as_ref().map(Vec::len).unwrap_or(0);
                if normalize_original_state(memo.state.as_deref()) == "ARCHIVED" {
                    progress.archived_count += 1;
                }
                let original_name = original_memo_name(&memo);
                let already_imported = !original_name.is_empty()
                    && (existing_names.contains(&original_name) || imported_original_names.contains(&original_name));
                if import_single_original_memo_inner(env, viewer, &memo, already_imported).await? {
                    if !original_name.is_empty() {
                        imported_original_names.insert(original_name);
                    }
                    progress.imported += 1;
                } else {
                    progress.skipped += 1;
                }
                progress.processed += 1;
                on_progress("progress", &progress)?;
            }

            if progress.truncated || next_page_token.is_empty() {
                break;
            }
            if next_page_token == previous_page_token {
                return Err(AppError::new(400, "Original Memos API returned a repeated page token"));
            }
            page_token = next_page_token;
        }
        if progress.truncated {
            break;
        }
    }

    progress.phase = "done".to_string();
    on_progress("progress", &progress)?;
    Ok(progress)
}

async fn import_single_original_memo(env: &Env, viewer: &Viewer, memo: &OriginalMemo) -> std::result::Result<bool, AppError> {
    let original_name = original_memo_name(memo);
    let already_imported = !original_name.is_empty() && has_imported_original_memo(env, viewer.id, &original_name).await?;
    import_single_original_memo_inner(env, viewer, memo, already_imported).await
}

async fn import_single_original_memo_inner(env: &Env, viewer: &Viewer, memo: &OriginalMemo, already_imported: bool) -> std::result::Result<bool, AppError> {
    let content = memo.content.as_deref().unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Ok(false);
    }
    let original_name = original_memo_name(memo);
    if already_imported {
        return Ok(false);
    }
    let now = unix_now();
    let created_ts = parse_original_timestamp(memo.create_time.as_ref(), now);
    let updated_ts = parse_original_timestamp(memo.update_time.as_ref(), created_ts);
    let uid = generate_uid("m");
    let attachments = memo.attachments.clone().unwrap_or_default();
    let relations = memo.relations.clone().unwrap_or_default();
    let mut payload = build_memo_payload_with_tags(&content, memo.tags.as_ref());
    payload["source"] = json!({
        "type": "usememos",
        "originalName": original_name,
        "creator": memo.creator.as_deref().unwrap_or(""),
        "attachmentCount": attachments.len(),
        "relationCount": relations.len(),
        "attachments": attachments,
        "relations": relations
    });
    if let Some(property) = &memo.property {
        payload["originalProperty"] = property.clone();
    }
    if let Some(parent) = &memo.parent {
        payload["originalParent"] = json!(parent);
    }
    if let Some(location) = &memo.location {
        payload["originalLocation"] = location.clone();
    }
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&[
            uid.into(),
            js_num(viewer.id),
            js_num(created_ts),
            js_num(updated_ts),
            normalize_original_state(memo.state.as_deref()).into(),
            content.into(),
            normalize_original_visibility(memo.visibility.as_deref()).into(),
            js_num(if memo.pinned.unwrap_or(false) { 1 } else { 0 }),
            payload.to_string().into(),
        ])?
        .run()
        .await?;
    Ok(true)
}

async fn existing_imported_original_names(env: &Env, creator_id: i64, memos: &[OriginalMemo]) -> std::result::Result<BTreeSet<String>, AppError> {
    let names: BTreeSet<String> = memos.iter()
        .map(original_memo_name)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return Ok(BTreeSet::new());
    }
    let mut existing = BTreeSet::new();
    let names: Vec<String> = names.into_iter().collect();
    for chunk in names.chunks(SQL_IN_CHUNK_SIZE) {
        let mut values = vec![js_num(creator_id)];
        values.extend(chunk.iter().map(|name| name.clone().into()));
        let rows = db(env)?.prepare(format!(
            "SELECT json_extract(payload, '$.source.originalName') AS original_name FROM memo WHERE creator_id = ? AND json_extract(payload, '$.source.type') = 'usememos' AND json_extract(payload, '$.source.originalName') IN ({})",
            placeholders(chunk.len())
        ))
            .bind(&values)?
            .all()
            .await?;
        let values: Vec<Value> = rows.results()?;
        existing.extend(values.into_iter()
            .filter_map(|row| row.get("original_name").and_then(Value::as_str).map(ToString::to_string)));
    }
    Ok(existing)
}

async fn has_imported_original_memo(env: &Env, creator_id: i64, original_name: &str) -> std::result::Result<bool, AppError> {
    let row: Option<i64> = db(env)?.prepare("SELECT id FROM memo WHERE creator_id = ? AND json_extract(payload, '$.source.type') = 'usememos' AND json_extract(payload, '$.source.originalName') = ? LIMIT 1")
        .bind(&[js_num(creator_id), original_name.into()])?
        .first(Some("id"))
        .await?;
    Ok(row.is_some())
}

fn original_memo_name(memo: &OriginalMemo) -> String {
    memo.name.as_deref().unwrap_or("").trim().to_string()
}

fn summarize_original_memos(memos: &[OriginalMemo], truncated: bool) -> MigrationSummary {
    let mut summary = MigrationSummary {
        memo_count: 0,
        attachment_count: 0,
        relation_count: 0,
        archived_count: 0,
        truncated,
    };
    for memo in memos {
        summary.memo_count += 1;
        summary.attachment_count += memo.attachments.as_ref().map(Vec::len).unwrap_or(0);
        summary.relation_count += memo.relations.as_ref().map(Vec::len).unwrap_or(0);
        if normalize_original_state(memo.state.as_deref()) == "ARCHIVED" {
            summary.archived_count += 1;
        }
    }
    summary
}

fn build_memo_payload_with_tags(content: &str, original_tags: Option<&Vec<String>>) -> Value {
    let mut payload = build_memo_payload(content);
    if let Some(tags) = original_tags {
        if let Some(existing) = payload.get_mut("tags").and_then(Value::as_array_mut) {
            for tag in tags.iter().map(|tag| tag.trim()).filter(|tag| !tag.is_empty()) {
                if !existing.iter().any(|value| value.as_str() == Some(tag)) {
                    existing.push(json!(tag));
                }
            }
        }
    }
    payload
}

fn parse_original_timestamp(value: Option<&Value>, fallback: i64) -> i64 {
    match value {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(fallback),
        Some(Value::String(text)) if !text.trim().is_empty() => {
            let parsed = js_sys::Date::parse(text);
            if parsed.is_finite() { (parsed / 1000.0).floor() as i64 } else { fallback }
        }
        _ => fallback,
    }
}

fn normalize_original_state(value: Option<&str>) -> String {
    let state = value.unwrap_or("NORMAL").to_ascii_uppercase().replace("STATE_", "");
    match state.as_str() {
        "" | "UNSPECIFIED" => "NORMAL".to_string(),
        "DELETED" => "ARCHIVED".to_string(),
        "ARCHIVED" => "ARCHIVED".to_string(),
        _ => "NORMAL".to_string(),
    }
}

fn normalize_original_visibility(value: Option<&str>) -> String {
    let visibility = value.unwrap_or("PRIVATE").to_ascii_uppercase().replace("VISIBILITY_", "");
    match visibility.as_str() {
        "PUBLIC" | "PROTECTED" | "PRIVATE" => visibility,
        _ => "PRIVATE".to_string(),
    }
}

fn sse_event<T: Serialize>(event: &str, data: &T) -> std::result::Result<String, AppError> {
    let data = serde_json::to_string(data).map_err(|error| AppError::new(500, error.to_string()))?;
    Ok(format!("event: {}\ndata: {}\n\n", event, data))
}

fn send_sse_chunk<T: Serialize>(sender: &mut mpsc::UnboundedSender<Vec<u8>>, event: &str, data: &T) -> std::result::Result<(), AppError> {
    let chunk = sse_event(event, data)?;
    sender.unbounded_send(chunk.into_bytes()).map_err(|_| AppError::new(500, "Migration progress stream closed"))
}

async fn connect_sse(req: &Request, env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let last_event_id = req.headers().get("Last-Event-ID").ok().flatten();
    let since_id = sse_since_id(last_event_id.as_deref(), url);
    let body = sse_connection_payload(env, viewer, since_id).await?;
    let mut response = Response::ok(body)?;
    response.headers_mut().set("Content-Type", "text/event-stream; charset=utf-8")?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    response.headers_mut().set("X-Accel-Buffering", "no")?;
    Ok(response)
}

fn sse_ready_payload(user_id: i64) -> std::result::Result<String, AppError> {
    Ok(format!("retry: 5000\n{}", sse_event("ready", &json!({ "userId": user_id }))?))
}

async fn sse_connection_payload(env: &Env, viewer: &Viewer, since_id: Option<i64>) -> std::result::Result<String, AppError> {
    let mut body = sse_ready_payload(viewer.id)?;
    for event in list_memo_events(env, viewer, since_id).await? {
        body.push_str(&memo_event_sse(&event)?);
    }
    Ok(body)
}

fn sse_since_id(last_event_id: Option<&str>, url: &Url) -> Option<i64> {
    last_event_id
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or_else(|| query_param(url, "since").and_then(|value| value.parse::<i64>().ok()))
        .filter(|value| *value > 0)
}

fn memo_event_sse(event: &DbMemoEvent) -> std::result::Result<String, AppError> {
    let mut payload = serde_json::from_str::<Value>(&event.payload).unwrap_or_else(|_| json!({}));
    if let Some(object) = payload.as_object_mut() {
        object.insert("id".to_string(), json!(event.id.to_string()));
        object.insert("type".to_string(), json!(event.event_type.clone()));
        object.insert("name".to_string(), json!(event.name.clone()));
        object.insert("visibility".to_string(), json!(event.visibility.clone()));
        object.insert("creatorId".to_string(), json!(event.creator_id));
    }
    format_sse_event(Some(event.id), Some(&event.event_type), &payload)
}

fn format_sse_event(id: Option<i64>, event: Option<&str>, data: &Value) -> std::result::Result<String, AppError> {
    let mut chunk = String::new();
    if let Some(id) = id {
        chunk.push_str(&format!("id: {}\n", id));
    }
    if let Some(event) = event {
        chunk.push_str(&format!("event: {}\n", event));
    }
    chunk.push_str(&format!("data: {}\n\n", serde_json::to_string(data).map_err(|error| AppError::new(500, error.to_string()))?));
    Ok(chunk)
}

async fn list_memo_events(env: &Env, viewer: &Viewer, since_id: Option<i64>) -> std::result::Result<Vec<DbMemoEvent>, AppError> {
    ensure_memo_event_table(env).await?;
    let rows = if let Some(id) = since_id {
        db(env)?.prepare("SELECT * FROM memo_event WHERE id > ? AND (? = 'ADMIN' OR visibility != 'PRIVATE' OR creator_id = ?) ORDER BY id ASC LIMIT 100")
            .bind(&[js_num(id), viewer.role.clone().into(), js_num(viewer.id)])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT * FROM memo_event WHERE created_ts >= ? AND (? = 'ADMIN' OR visibility != 'PRIVATE' OR creator_id = ?) ORDER BY id ASC LIMIT 100")
            .bind(&[js_num(unix_now() - 60), viewer.role.clone().into(), js_num(viewer.id)])?
            .all()
            .await?
    };
    Ok(rows.results()?)
}

async fn emit_memo_event(env: &Env, event_type: &str, memo: &DbMemo) {
    emit_memo_change(env, event_type, memo, json!({})).await;
}

async fn emit_memo_change(env: &Env, event_type: &str, memo: &DbMemo, detail: Value) {
    if let Err(error) = record_memo_event_with_detail(env, event_type, memo, detail.clone()).await {
        console_log!("memo event record failed: {}", error.message);
    }
    if let Err(error) = fire_memo_webhooks(env, event_type, memo, detail).await {
        console_log!("memo webhook dispatch failed: {}", error.message);
    }
}

async fn record_memo_event_with_detail(env: &Env, event_type: &str, memo: &DbMemo, detail: Value) -> std::result::Result<(), AppError> {
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

async fn emit_bulk_memo_events(
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

fn memo_event_payload(event_type: &str, memo: &DbMemo) -> Value {
    json!({
        "type": event_type,
        "name": format!("memos/{}", memo.uid),
        "visibility": memo.visibility,
        "creatorId": memo.creator_id
    })
}

fn memo_event_payload_with_detail(event_type: &str, memo: &DbMemo, detail: Value) -> Value {
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

fn memo_webhook_body(event_type: &str, memo: &DbMemo, timestamp: i64, detail: Value) -> Value {
    json!({
        "event": event_type,
        "timestamp": timestamp,
        "payload": {
            "memo": public_memo(memo.clone()),
            "detail": detail
        }
    })
}

async fn fire_memo_webhooks(env: &Env, event_type: &str, memo: &DbMemo, detail: Value) -> std::result::Result<(), AppError> {
    let rows = db(env)?.prepare("SELECT * FROM webhook WHERE creator_id = ? AND row_status = 'NORMAL' ORDER BY id")
        .bind(&[js_num(memo.creator_id)])?
        .all()
        .await?;
    let webhooks: Vec<DbWebhook> = rows.results()?;
    if webhooks.is_empty() {
        return Ok(());
    }
    let body = memo_webhook_body(event_type, memo, unix_now(), detail).to_string();
    for webhook in webhooks {
        if let Err(error) = send_and_record_webhook(env, webhook.id, memo.creator_id, &webhook.url, event_type, &body).await {
            console_log!("webhook delivery failed: {}", error.message);
        }
    }
    Ok(())
}

async fn prune_memo_events(env: &Env, retention_days: i64) -> std::result::Result<i64, AppError> {
    ensure_memo_event_table(env).await?;
    let cutoff = memo_event_retention_cutoff(unix_now(), retention_days);
    db(env)?.prepare("DELETE FROM memo_event WHERE created_ts < ?")
        .bind(&[js_num(cutoff)])?
        .run()
        .await?;
    let count: Option<i64> = db(env)?.prepare("SELECT changes() AS count")
        .first(Some("count"))
        .await?;
    Ok(count.unwrap_or(0))
}

fn memo_event_retention_cutoff(now: i64, retention_days: i64) -> i64 {
    now - retention_days.max(0) * 24 * 60 * 60
}

async fn ensure_memo_event_table(env: &Env) -> std::result::Result<(), AppError> {
    db(env)?.prepare("CREATE TABLE IF NOT EXISTS memo_event (id INTEGER PRIMARY KEY AUTOINCREMENT, created_ts INTEGER NOT NULL DEFAULT (strftime('%s', 'now')), event_type TEXT NOT NULL, name TEXT NOT NULL, visibility TEXT NOT NULL DEFAULT 'PRIVATE', creator_id INTEGER NOT NULL, payload TEXT NOT NULL DEFAULT '{}')")
        .run()
        .await?;
    db(env)?.prepare("CREATE INDEX IF NOT EXISTS idx_memo_event_created ON memo_event(created_ts, id)")
        .run()
        .await?;
    db(env)?.prepare("CREATE INDEX IF NOT EXISTS idx_memo_event_visibility ON memo_event(visibility, creator_id, id)")
        .run()
        .await?;
    Ok(())
}

async fn record_migration_audit(env: &Env, viewer: &Viewer, options: &MigrationOptions, progress: &MigrationProgress) {
    record_audit(env, Some(viewer), "migration.usememos.import", "usememos", json!({
        "baseUrl": options.base_url,
        "imported": progress.imported,
        "skipped": progress.skipped,
        "memoCount": progress.memo_count,
        "attachmentCount": progress.attachment_count,
        "relationCount": progress.relation_count,
        "archivedCount": progress.archived_count,
        "truncated": progress.truncated
    })).await;
}

async fn record_migration_start_audit(env: &Env, viewer: &Viewer, options: &MigrationOptions) {
    record_audit(env, Some(viewer), "migration.usememos.start", "usememos", json!({
        "baseUrl": options.base_url,
        "includeArchived": options.include_archived
    })).await;
}

async fn record_migration_error_audit(env: &Env, viewer: &Viewer, options: &MigrationOptions, message: &str) {
    record_audit(env, Some(viewer), "migration.usememos.error", "usememos", json!({
        "baseUrl": options.base_url,
        "error": message
    })).await;
}
