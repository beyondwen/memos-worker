use super::*;

pub(crate) async fn list_comments(
    env: &Env,
    viewer: &Viewer,
    parent_uid: &str,
) -> std::result::Result<Response, AppError> {
    let parent = get_memo_by_uid(env, parent_uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&parent, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let rows = db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN memo_relation ON memo_relation.related_memo_id = memo.id JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo_relation.memo_id = ? AND memo_relation.type = 'COMMENT' AND memo.row_status = 'NORMAL' ORDER BY memo.created_ts ASC")
        .bind(&[js_num(parent.id)])?
        .all()
        .await?;
    let memos: Vec<DbMemo> = rows.results()?;
    let public: Vec<PublicMemo> = memos.into_iter().map(public_memo).collect();
    json_response(json!({ "memos": public }), 200).map_err(AppError::from)
}

pub(crate) async fn create_comment(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    parent_uid: &str,
) -> std::result::Result<Response, AppError> {
    let parent = get_memo_by_uid(env, parent_uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&parent, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
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
    let uid = generate_uid("m")?;
    let now = unix_now();
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, payload) VALUES (?, ?, ?, ?, ?, 'PROTECTED', ?)")
        .bind(&[uid.clone().into(), js_num(viewer.id), js_num(now), js_num(now), content.into(), build_memo_payload("").to_string().into()])?
        .run()
        .await?;
    let comment = get_memo_by_uid(env, &uid)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create comment"))?;
    db(env)?.prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'COMMENT')")
        .bind(&[js_num(parent.id), js_num(comment.id)])?
        .run()
        .await?;
    emit_memo_change(
        env,
        "memo.created",
        &comment,
        json!({ "parentMemoUid": parent.uid.clone() }),
    )
    .await;
    emit_memo_change(
        env,
        "memo.comment.created",
        &parent,
        json!({ "comment": public_memo(comment.clone()) }),
    )
    .await;
    let comment = memo_with_attachments(env, comment).await?;
    json_response(json!({ "memo": comment }), 201).map_err(AppError::from)
}
