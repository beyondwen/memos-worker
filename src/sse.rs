use super::*;

pub(crate) fn sse_event<T: Serialize>(
    event: &str,
    data: &T,
) -> std::result::Result<String, AppError> {
    let data =
        serde_json::to_string(data).map_err(|error| AppError::new(500, error.to_string()))?;
    Ok(format!("event: {}\ndata: {}\n\n", event, data))
}

pub(crate) fn send_sse_chunk<T: Serialize>(
    sender: &mut mpsc::UnboundedSender<Vec<u8>>,
    event: &str,
    data: &T,
) -> std::result::Result<(), AppError> {
    let chunk = sse_event(event, data)?;
    sender
        .unbounded_send(chunk.into_bytes())
        .map_err(|_| AppError::new(500, "Migration progress stream closed"))
}

pub(crate) async fn connect_sse(
    req: &Request,
    env: &Env,
    url: &Url,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let last_event_id = req.headers().get("Last-Event-ID").ok().flatten();
    let since_id = sse_since_id(last_event_id.as_deref(), url);
    let body = sse_connection_payload(env, viewer, since_id).await?;
    let mut response = Response::ok(body)?;
    response
        .headers_mut()
        .set("Content-Type", "text/event-stream; charset=utf-8")?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    response.headers_mut().set("X-Accel-Buffering", "no")?;
    Ok(response)
}

pub(crate) fn sse_ready_payload(user_id: i64) -> std::result::Result<String, AppError> {
    Ok(format!(
        "retry: 5000\n{}",
        sse_event("ready", &json!({ "userId": user_id }))?
    ))
}

pub(crate) async fn sse_connection_payload(
    env: &Env,
    viewer: &Viewer,
    since_id: Option<i64>,
) -> std::result::Result<String, AppError> {
    let mut body = sse_ready_payload(viewer.id)?;
    for event in list_memo_events(env, viewer, since_id).await? {
        body.push_str(&memo_event_sse(&event)?);
    }
    Ok(body)
}

pub(crate) fn sse_since_id(last_event_id: Option<&str>, url: &Url) -> Option<i64> {
    last_event_id
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or_else(|| query_param(url, "since").and_then(|value| value.parse::<i64>().ok()))
        .filter(|value| *value > 0)
}

pub(crate) fn memo_event_sse(event: &DbMemoEvent) -> std::result::Result<String, AppError> {
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

pub(crate) fn format_sse_event(
    id: Option<i64>,
    event: Option<&str>,
    data: &Value,
) -> std::result::Result<String, AppError> {
    let mut chunk = String::new();
    if let Some(id) = id {
        chunk.push_str(&format!("id: {}\n", id));
    }
    if let Some(event) = event {
        chunk.push_str(&format!("event: {}\n", event));
    }
    chunk.push_str(&format!(
        "data: {}\n\n",
        serde_json::to_string(data).map_err(|error| AppError::new(500, error.to_string()))?
    ));
    Ok(chunk)
}

pub(crate) async fn list_memo_events(
    env: &Env,
    viewer: &Viewer,
    since_id: Option<i64>,
) -> std::result::Result<Vec<DbMemoEvent>, AppError> {
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
