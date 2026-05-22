use super::*;

pub(crate) async fn list_memos(
    env: &Env,
    url: &Url,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let db = db(env)?;
    let limit = url
        .query_pairs()
        .find(|(key, _)| key == "page_size" || key == "pageSize")
        .and_then(|(_, value)| value.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 200);
    let offset = query_param(url, "page_token")
        .or_else(|| query_param(url, "pageToken"))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
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
            "EXISTS (SELECT 1 FROM json_each(memo.payload, '$.tags') WHERE value = ?)".to_string(),
        );
        values.push(tag.into());
    }
    if let Some(search) =
        extract_content_contains_filter(url).filter(|value| !value.trim().is_empty())
    {
        where_sql.push("memo.content LIKE ? ESCAPE '\\'".to_string());
        values.push(format!("%{}%", escape_like(&search)).into());
    }
    values.push(js_num(limit + 1));
    values.push(js_num(offset));
    let rows = db.prepare(format!(
        "SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE {} ORDER BY memo.pinned DESC, memo.created_ts DESC, memo.id DESC LIMIT ? OFFSET ?",
        where_sql.join(" AND ")
    ))
        .bind(&values)?
        .all()
        .await?;
    let memos: Vec<DbMemo> = rows.results()?;
    let has_more = memos.len() as i64 > limit;
    let mut public = Vec::new();
    for memo in memos.into_iter().take(limit as usize) {
        public.push(memo_with_attachments(env, memo).await?);
    }
    let next_page_token = if has_more {
        (offset + limit).to_string()
    } else {
        String::new()
    };
    json_response(
        json!({ "memos": public, "nextPageToken": next_page_token }),
        200,
    )
    .map_err(AppError::from)
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
    let payload = build_memo_payload(&content);
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, payload) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&[uid.clone().into(), js_num(viewer.id), js_num(now), js_num(now), content.into(), visibility.into(), payload.to_string().into()])?
        .run()
        .await?;
    let memo = get_memo_by_uid(env, &uid)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create memo"))?;
    emit_memo_event(env, "memo.created", &memo).await;
    let memo = memo_with_attachments(env, memo).await?;
    json_response(json!({ "memo": memo }), 201).map_err(AppError::from)
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
