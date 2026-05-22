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
    if let Err(error) =
        record_comment_inbox(env, viewer.id, parent.creator_id, &parent.uid, &comment.uid).await
    {
        console_log!("comment inbox record failed: {}", error.message);
    }
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

pub(crate) async fn list_reactions(
    env: &Env,
    viewer: &Viewer,
    uid: &str,
) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    list_reactions_for_memo(env, memo.id).await
}

pub(crate) async fn upsert_reaction(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
    uid: &str,
) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let reaction_type = body
        .get("reactionType")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if reaction_type.is_empty() {
        return Err(AppError::new(400, "reactionType is required"));
    }
    db(env)?.prepare("INSERT INTO reaction (created_ts, creator_id, content_type, content_id, reaction_type) VALUES (?, ?, 'MEMO', ?, ?) ON CONFLICT (creator_id, content_type, content_id, reaction_type) DO NOTHING")
        .bind(&[js_num(unix_now()), js_num(viewer.id), js_num(memo.id), reaction_type.into()])?
        .run()
        .await?;
    emit_memo_change(
        env,
        "reaction.upserted",
        &memo,
        json!({ "reactionType": reaction_type, "actorId": viewer.id }),
    )
    .await;
    list_reactions_for_memo(env, memo.id).await
}

pub(crate) async fn delete_reaction(
    env: &Env,
    viewer: &Viewer,
    uid: &str,
    reaction_id: &str,
) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    let id = reaction_id
        .parse::<i64>()
        .map_err(|_| AppError::new(400, "Invalid reaction ID"))?;
    let row: Option<Value> = db(env)?.prepare("SELECT id, creator_id FROM reaction WHERE id = ? AND content_type = 'MEMO' AND content_id = ?")
        .bind(&[js_num(id), js_num(memo.id)])?
        .first(None)
        .await?;
    let row = row.ok_or_else(|| AppError::new(404, "Reaction not found"))?;
    let creator_id = row
        .get("creator_id")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if viewer.role != "ADMIN" && creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    db(env)?
        .prepare("DELETE FROM reaction WHERE id = ?")
        .bind(&[js_num(id)])?
        .run()
        .await?;
    emit_memo_change(
        env,
        "reaction.deleted",
        &memo,
        json!({ "reactionId": id, "actorId": viewer.id }),
    )
    .await;
    list_reactions_for_memo(env, memo.id).await
}

async fn list_reactions_for_memo(
    env: &Env,
    memo_id: i64,
) -> std::result::Result<Response, AppError> {
    let rows = db(env)?.prepare("SELECT reaction.id, reaction.created_ts, reaction.reaction_type, reaction.creator_id, \"user\".username AS creator_username FROM reaction JOIN \"user\" ON \"user\".id = reaction.creator_id WHERE reaction.content_type = 'MEMO' AND reaction.content_id = ? ORDER BY reaction.created_ts ASC")
        .bind(&[js_num(memo_id)])?
        .all()
        .await?;
    let reactions: Vec<DbReaction> = rows.results()?;
    let payload: Vec<Value> = reactions
        .into_iter()
        .map(|reaction| {
            json!({
                "id": reaction.id,
                "reactionType": reaction.reaction_type,
                "creator": { "id": reaction.creator_id, "username": reaction.creator_username },
                "createdTs": reaction.created_ts
            })
        })
        .collect();
    json_response(json!({ "reactions": payload }), 200).map_err(AppError::from)
}
