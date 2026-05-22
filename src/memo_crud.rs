use super::*;

pub(crate) async fn list_memos(
    env: &Env,
    url: &Url,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let db = db(env)?;
    ensure_memo_index_tables(env).await?;
    let limit = url
        .query_pairs()
        .find(|(key, _)| key == "page_size" || key == "pageSize")
        .and_then(|(_, value)| value.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 200);
    let state = query_param(url, "state")
        .or_else(|| extract_filter_value(url, "rowStatus"))
        .or_else(|| extract_filter_value(url, "row_status"))
        .unwrap_or_else(|| "NORMAL".to_string());
    let state = normalize_state(&state)?;
    let mut where_sql = vec!["memo.row_status = ?".to_string()];
    let mut values = vec![state.into()];
    if viewer.role != "ADMIN" {
        where_sql.push("(memo.visibility != 'PRIVATE' OR memo.creator_id = ?)".to_string());
        values.push(js_num(viewer.id));
    }
    if let Some(visibility) =
        query_param(url, "visibility").filter(|value| !value.trim().is_empty())
    {
        where_sql.push("memo.visibility = ?".to_string());
        values.push(normalize_visibility(&visibility)?.into());
    }
    if let Some(tag) = query_param(url, "tag").filter(|value| !value.trim().is_empty()) {
        where_sql.push(
            "EXISTS (SELECT 1 FROM memo_tag WHERE memo_tag.memo_id = memo.id AND memo_tag.tag = ?)"
                .to_string(),
        );
        values.push(tag.into());
    }
    if let Some(search) =
        extract_content_contains_filter(url).filter(|value| !value.trim().is_empty())
    {
        where_sql.push("EXISTS (SELECT 1 FROM memo_search WHERE memo_search.memo_id = memo.id AND memo_search.content LIKE ? ESCAPE '\\')".to_string());
        values.push(format!("%{}%", escape_like(&search)).into());
    }
    if let Some(start_ts) = memo_created_after_ts(url)? {
        where_sql.push("memo.created_ts >= ?".to_string());
        values.push(js_num(start_ts));
    }
    if let Some(end_ts) = memo_created_before_exclusive_ts(url)? {
        where_sql.push("memo.created_ts < ?".to_string());
        values.push(js_num(end_ts));
    }
    if let Some(cursor) = memo_page_cursor(url) {
        where_sql.push("(memo.pinned < ? OR (memo.pinned = ? AND memo.created_ts < ?) OR (memo.pinned = ? AND memo.created_ts = ? AND memo.id < ?))".to_string());
        values.extend([
            js_num(cursor.pinned),
            js_num(cursor.pinned),
            js_num(cursor.created_ts),
            js_num(cursor.pinned),
            js_num(cursor.created_ts),
            js_num(cursor.id),
        ]);
    }
    values.push(js_num(limit + 1));
    let rows = db.prepare(format!(
        "SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE {} ORDER BY memo.pinned DESC, memo.created_ts DESC, memo.id DESC LIMIT ?",
        where_sql.join(" AND ")
    ))
        .bind(&values)?
        .all()
        .await?;
    let memos: Vec<DbMemo> = rows.results()?;
    let has_more = memos.len() as i64 > limit;
    let page_memos: Vec<DbMemo> = memos.into_iter().take(limit as usize).collect();
    let memo_ids: Vec<i64> = page_memos.iter().map(|memo| memo.id).collect();
    let attachments = list_attachments_for_memos(env, &memo_ids).await?;
    let next_page_token = if has_more {
        page_memos
            .last()
            .map(build_memo_page_token)
            .unwrap_or_default()
    } else {
        String::new()
    };
    let public = public_memos_with_attachments(page_memos, attachments);
    json_response(
        json!({ "memos": public, "nextPageToken": next_page_token }),
        200,
    )
    .map_err(AppError::from)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoPageCursor {
    pub(crate) pinned: i64,
    pub(crate) created_ts: i64,
    pub(crate) id: i64,
}

pub(crate) fn memo_page_cursor(url: &Url) -> Option<MemoPageCursor> {
    let token = query_param(url, "page_token").or_else(|| query_param(url, "pageToken"))?;
    parse_memo_page_token(&token)
}

pub(crate) fn parse_memo_page_token(token: &str) -> Option<MemoPageCursor> {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    Some(MemoPageCursor {
        pinned: parts[0].parse::<i64>().ok()?.clamp(0, 1),
        created_ts: parts[1].parse::<i64>().ok()?,
        id: parts[2].parse::<i64>().ok()?,
    })
}

pub(crate) fn build_memo_page_token(memo: &DbMemo) -> String {
    format!("{}:{}:{}", memo.pinned, memo.created_ts, memo.id)
}

pub(crate) fn memo_created_after_ts(url: &Url) -> std::result::Result<Option<i64>, AppError> {
    query_param(url, "created_after")
        .or_else(|| query_param(url, "createdAfter"))
        .map(|value| memo_date_start_ts(&value))
        .transpose()
}

pub(crate) fn memo_created_before_exclusive_ts(
    url: &Url,
) -> std::result::Result<Option<i64>, AppError> {
    query_param(url, "created_before")
        .or_else(|| query_param(url, "createdBefore"))
        .map(|value| memo_date_start_ts(&value).map(|ts| ts + 86_400))
        .transpose()
}

fn memo_date_start_ts(value: &str) -> std::result::Result<i64, AppError> {
    chrono::NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|date| date.and_utc().timestamp())
        .ok_or_else(|| AppError::new(400, "Invalid memo date filter"))
}

pub(crate) async fn create_memo(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if content.is_empty() {
        return Err(AppError::new(400, "Content is required"));
    }
    let visibility = normalize_visibility(
        body.get("visibility")
            .and_then(Value::as_str)
            .unwrap_or("PRIVATE"),
    )?;
    let uid = generate_uid("m")?;
    let now = unix_now();
    let created_ts = memo_created_ts_from_body(&body, now)?;
    let payload = build_memo_payload(&content);
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, payload) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&[uid.clone().into(), js_num(viewer.id), js_num(created_ts), js_num(now), content.into(), visibility.into(), payload.to_string().into()])?
        .run()
        .await?;
    let memo = get_memo_by_uid(env, &uid)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create memo"))?;
    sync_memo_index(env, &memo).await?;
    emit_memo_event(env, "memo.created", &memo).await;
    let memo = memo_with_attachments(env, memo).await?;
    json_response(json!({ "memo": memo }), 201).map_err(AppError::from)
}

pub(crate) fn memo_created_ts_from_body(
    body: &Value,
    fallback: i64,
) -> std::result::Result<i64, AppError> {
    let Some(value) = body
        .get("createdTs")
        .or_else(|| body.get("created_ts"))
        .or_else(|| body.get("createdAt"))
    else {
        return Ok(fallback);
    };
    if value.is_null() {
        return Ok(fallback);
    }
    let ts = if let Some(ts) = value.as_i64() {
        ts
    } else if let Some(raw) = value.as_str() {
        chrono::NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%dT%H:%M")
            .ok()
            .or_else(|| {
                chrono::NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
                    .ok()
                    .and_then(|date| date.and_hms_opt(0, 0, 0))
            })
            .map(|date| date.and_utc().timestamp())
            .ok_or_else(|| AppError::new(400, "Invalid memo created date"))?
    } else {
        return Err(AppError::new(400, "Invalid memo created date"));
    };
    if !(0..=4_102_444_800).contains(&ts) {
        return Err(AppError::new(400, "Invalid memo created date"));
    }
    Ok(ts)
}

pub(crate) async fn memo_subroute(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    raw: &str,
    method: Method,
    url: &Url,
) -> std::result::Result<Response, AppError> {
    let parts: Vec<&str> = raw.split('/').collect();
    let uid = parts[0];
    if parts.len() > 1 {
        match memo_child_route(&parts, &method) {
            MemoChildRoute::ListComments => return list_comments(env, viewer, uid).await,
            MemoChildRoute::CreateComment => return create_comment(req, env, viewer, uid).await,
            MemoChildRoute::GetRelations => return get_relations(env, viewer, uid).await,
            MemoChildRoute::SuggestRelations => {
                return suggest_memo_relations(env, viewer, uid).await
            }
            MemoChildRoute::SetRelations => return set_relations(req, env, viewer, uid).await,
            MemoChildRoute::Unsupported => {
                return Err(AppError::new(404, "Memo subroute not found"))
            }
        }
    }
    match method {
        Method::Get => {
            let memo = get_memo_by_uid(env, uid)
                .await?
                .ok_or_else(|| AppError::new(404, "Memo not found"))?;
            if !can_read(&memo, viewer) {
                return Err(AppError::new(403, "Forbidden"));
            }
            let memo = memo_with_attachments(env, memo).await?;
            json_response(json!({ "memo": memo }), 200).map_err(AppError::from)
        }
        Method::Patch => update_memo(req, env, viewer, uid).await,
        Method::Delete => {
            let memo = get_memo_by_uid(env, uid)
                .await?
                .ok_or_else(|| AppError::new(404, "Memo not found"))?;
            if !can_write(&memo, viewer) {
                return Err(AppError::new(403, "Forbidden"));
            }
            if url.query_pairs().any(|(k, v)| k == "purge" && v == "true") {
                purge_ids(env, &[memo.id]).await?;
                emit_memo_event(env, "memo.deleted", &memo).await;
            } else {
                db(env)?
                    .prepare("UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id = ?")
                    .bind(&[js_num(unix_now()), js_num(memo.id)])?
                    .run()
                    .await?;
                let archived = DbMemo {
                    row_status: "ARCHIVED".to_string(),
                    updated_ts: unix_now(),
                    ..memo.clone()
                };
                emit_memo_event(env, "memo.archived", &archived).await;
            }
            json_response(json!({ "ok": true }), 200).map_err(AppError::from)
        }
        _ => Err(AppError::new(405, "Method not allowed")),
    }
}

pub(crate) fn memo_child_route(parts: &[&str], method: &Method) -> MemoChildRoute {
    match (parts.get(1).copied(), method) {
        (Some("comments"), Method::Get) => MemoChildRoute::ListComments,
        (Some("comments"), Method::Post) => MemoChildRoute::CreateComment,
        (Some("relations"), Method::Get) if parts.len() == 2 => MemoChildRoute::GetRelations,
        (Some("relations"), Method::Post) if parts.get(2) == Some(&"suggest") => {
            MemoChildRoute::SuggestRelations
        }
        (Some("relations"), Method::Patch) if parts.len() == 2 => MemoChildRoute::SetRelations,
        _ => MemoChildRoute::Unsupported,
    }
}

pub(crate) async fn update_memo(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    uid: &str,
) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_write(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let content = body
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or(&memo.content)
        .trim()
        .to_string();
    let visibility = normalize_visibility(
        body.get("visibility")
            .and_then(Value::as_str)
            .unwrap_or(&memo.visibility),
    )?;
    let row_status = normalize_state(
        body.get("rowStatus")
            .or_else(|| body.get("row_status"))
            .and_then(Value::as_str)
            .unwrap_or(&memo.row_status),
    )?;
    let pinned = body
        .get("pinned")
        .and_then(Value::as_bool)
        .map(|value| if value { 1 } else { 0 })
        .unwrap_or(memo.pinned);
    let payload = build_memo_payload(&content);
    db(env)?.prepare("UPDATE memo SET updated_ts = ?, content = ?, visibility = ?, pinned = ?, row_status = ?, payload = ? WHERE id = ?")
        .bind(&[js_num(unix_now()), content.into(), visibility.into(), js_num(pinned), row_status.into(), payload.to_string().into(), js_num(memo.id)])?
        .run()
        .await?;
    let updated = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(500, "Memo disappeared"))?;
    sync_memo_index(env, &updated).await?;
    let event_type = if memo.row_status != updated.row_status {
        if updated.row_status == "ARCHIVED" {
            "memo.archived"
        } else {
            "memo.restored"
        }
    } else {
        "memo.updated"
    };
    emit_memo_event(env, event_type, &updated).await;
    let updated = memo_with_attachments(env, updated).await?;
    json_response(json!({ "memo": updated }), 200).map_err(AppError::from)
}
