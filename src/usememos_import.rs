use super::*;

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
            uid.clone().into(),
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
    if let Some(inserted) = get_memo_by_uid(env, &uid).await? {
        sync_memo_index(env, &inserted).await?;
    }
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
