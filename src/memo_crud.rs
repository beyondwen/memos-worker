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
    let uid = generate_uid("m");
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
            MemoChildRoute::ListReactions => return list_reactions(env, viewer, uid).await,
            MemoChildRoute::UpsertReaction => return upsert_reaction(req, env, viewer, uid).await,
            MemoChildRoute::DeleteReaction(reaction_id) => {
                return delete_reaction(env, viewer, uid, reaction_id).await
            }
            MemoChildRoute::GetRelations => return get_relations(env, viewer, uid).await,
            MemoChildRoute::SuggestRelations => {
                return suggest_memo_relations(env, viewer, uid).await
            }
            MemoChildRoute::SetRelations => return set_relations(req, env, viewer, uid).await,
            MemoChildRoute::ListShares => return list_shares(env, viewer, uid).await,
            MemoChildRoute::CreateShare => return create_share(req, env, viewer, uid).await,
            MemoChildRoute::DeleteShare(share_id) => {
                return delete_share(env, viewer, uid, share_id).await
            }
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

pub(crate) fn memo_child_route<'a>(parts: &'a [&'a str], method: &Method) -> MemoChildRoute<'a> {
    match (parts.get(1).copied(), method) {
        (Some("comments"), Method::Get) => MemoChildRoute::ListComments,
        (Some("comments"), Method::Post) => MemoChildRoute::CreateComment,
        (Some("reactions"), Method::Get) => MemoChildRoute::ListReactions,
        (Some("reactions"), Method::Post) => MemoChildRoute::UpsertReaction,
        (Some("reactions"), Method::Delete) if parts.len() > 2 => {
            MemoChildRoute::DeleteReaction(parts[2])
        }
        (Some("relations"), Method::Get) if parts.len() == 2 => MemoChildRoute::GetRelations,
        (Some("relations"), Method::Post) if parts.get(2) == Some(&"suggest") => {
            MemoChildRoute::SuggestRelations
        }
        (Some("relations"), Method::Patch) if parts.len() == 2 => MemoChildRoute::SetRelations,
        (Some("shares"), Method::Get) => MemoChildRoute::ListShares,
        (Some("shares"), Method::Post) => MemoChildRoute::CreateShare,
        (Some("shares"), Method::Delete) if parts.len() > 2 => {
            MemoChildRoute::DeleteShare(parts[2])
        }
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

pub(crate) async fn bulk_memos(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let action = body
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_uppercase();
    let mut seen = BTreeSet::new();
    let uids: Vec<String> = body
        .get("memoUids")
        .and_then(Value::as_array)
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|uid| !uid.is_empty())
        .filter(|uid| seen.insert(uid.to_string()))
        .map(ToString::to_string)
        .take(200)
        .collect();
    if uids.is_empty() {
        return Err(AppError::new(400, "memoUids is required"));
    }
    let memos = get_memos_by_uids(env, viewer, &uids).await?;
    let ids: Vec<i64> = memos.iter().map(|memo| memo.id).collect();
    let result =
        json!({ "updated": 0, "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) });
    if ids.is_empty() {
        return json_response(result, 200).map_err(AppError::from);
    }
    let placeholders = placeholders(ids.len());
    let now = unix_now();
    match action.as_str() {
        "ARCHIVE" => {
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&bind_with_first(now, &ids))?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "ARCHIVE",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                Some("ARCHIVED"),
                None,
            )
            .await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "RESTORE" => {
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET row_status = 'NORMAL', updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&bind_with_first(now, &ids))?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "RESTORE",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                Some("NORMAL"),
                None,
            )
            .await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "DELETE" => {
            purge_ids(env, &ids).await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "DELETE",
                0,
                ids.len(),
                uids.len().saturating_sub(ids.len()),
                now,
                None,
                None,
            )
            .await;
            json_response(json!({ "updated": 0, "deleted": ids.len(), "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "VISIBILITY" => {
            let visibility =
                normalize_visibility(body.get("visibility").and_then(Value::as_str).unwrap_or(""))?;
            let mut values = vec![visibility.clone().into(), js_num(now)];
            values.extend(ids.iter().map(|id| js_num(*id)));
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET visibility = ?, updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&values)?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "VISIBILITY",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                None,
                Some(&visibility),
            )
            .await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        _ => Err(AppError::new(400, "Invalid bulk action")),
    }
}
