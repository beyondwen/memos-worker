use super::*;

pub(crate) async fn import_original_memos(
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

pub(crate) async fn import_original_memos_streaming<F>(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    mut on_progress: F,
) -> std::result::Result<MigrationProgress, AppError>
where
    F: FnMut(&str, &MigrationProgress) -> std::result::Result<(), AppError>,
{
    let states = if options.include_archived {
        vec!["NORMAL", "ARCHIVED"]
    } else {
        vec!["NORMAL"]
    };
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

            let mut url = Url::parse(&format!("{}/api/v1/memos", options.base_url))
                .map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
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
                    && (existing_names.contains(&original_name)
                        || imported_original_names.contains(&original_name));
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
                return Err(AppError::new(
                    400,
                    "Original Memos API returned a repeated page token",
                ));
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

pub(crate) async fn import_single_original_memo(
    env: &Env,
    viewer: &Viewer,
    memo: &OriginalMemo,
) -> std::result::Result<bool, AppError> {
    let original_name = original_memo_name(memo);
    let already_imported = !original_name.is_empty()
        && has_imported_original_memo(env, viewer.id, &original_name).await?;
    import_single_original_memo_inner(env, viewer, memo, already_imported).await
}

pub(crate) async fn import_single_original_memo_inner(
    env: &Env,
    viewer: &Viewer,
    memo: &OriginalMemo,
    already_imported: bool,
) -> std::result::Result<bool, AppError> {
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
    let uid = generate_uid("m")?;
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

pub(crate) async fn existing_imported_original_names(
    env: &Env,
    creator_id: i64,
    memos: &[OriginalMemo],
) -> std::result::Result<BTreeSet<String>, AppError> {
    let names: BTreeSet<String> = memos
        .iter()
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
        existing.extend(values.into_iter().filter_map(|row| {
            row.get("original_name")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }));
    }
    Ok(existing)
}

pub(crate) async fn has_imported_original_memo(
    env: &Env,
    creator_id: i64,
    original_name: &str,
) -> std::result::Result<bool, AppError> {
    let row: Option<i64> = db(env)?.prepare("SELECT id FROM memo WHERE creator_id = ? AND json_extract(payload, '$.source.type') = 'usememos' AND json_extract(payload, '$.source.originalName') = ? LIMIT 1")
        .bind(&[js_num(creator_id), original_name.into()])?
        .first(Some("id"))
        .await?;
    Ok(row.is_some())
}

pub(crate) fn original_memo_name(memo: &OriginalMemo) -> String {
    memo.name.as_deref().unwrap_or("").trim().to_string()
}

pub(crate) fn summarize_original_memos(
    memos: &[OriginalMemo],
    truncated: bool,
) -> MigrationSummary {
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

pub(crate) fn build_memo_payload_with_tags(
    content: &str,
    original_tags: Option<&Vec<String>>,
) -> Value {
    let mut payload = build_memo_payload(content);
    if let Some(tags) = original_tags {
        if let Some(existing) = payload.get_mut("tags").and_then(Value::as_array_mut) {
            for tag in tags
                .iter()
                .map(|tag| tag.trim())
                .filter(|tag| !tag.is_empty())
            {
                if !existing.iter().any(|value| value.as_str() == Some(tag)) {
                    existing.push(json!(tag));
                }
            }
        }
    }
    payload
}

pub(crate) fn parse_original_timestamp(value: Option<&Value>, fallback: i64) -> i64 {
    match value {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(fallback),
        Some(Value::String(text)) if !text.trim().is_empty() => {
            let parsed = js_sys::Date::parse(text);
            if parsed.is_finite() {
                (parsed / 1000.0).floor() as i64
            } else {
                fallback
            }
        }
        _ => fallback,
    }
}

pub(crate) fn normalize_original_state(value: Option<&str>) -> String {
    let state = value
        .unwrap_or("NORMAL")
        .to_ascii_uppercase()
        .replace("STATE_", "");
    match state.as_str() {
        "" | "UNSPECIFIED" => "NORMAL".to_string(),
        "DELETED" => "ARCHIVED".to_string(),
        "ARCHIVED" => "ARCHIVED".to_string(),
        _ => "NORMAL".to_string(),
    }
}

pub(crate) fn normalize_original_visibility(value: Option<&str>) -> String {
    let visibility = value
        .unwrap_or("PRIVATE")
        .to_ascii_uppercase()
        .replace("VISIBILITY_", "");
    match visibility.as_str() {
        "PUBLIC" | "PROTECTED" | "PRIVATE" => visibility,
        _ => "PRIVATE".to_string(),
    }
}
