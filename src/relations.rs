use super::*;

pub(crate) async fn get_relations(
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
    let refs = db(env)?.prepare("SELECT memo.uid, memo.content, memo_relation.type FROM memo_relation JOIN memo ON memo.id = memo_relation.related_memo_id WHERE memo_relation.memo_id = ? AND memo_relation.type = 'REFERENCE'")
        .bind(&[js_num(memo.id)])?
        .all()
        .await?;
    let back_refs = db(env)?.prepare("SELECT memo.uid, memo.content, memo_relation.type FROM memo_relation JOIN memo ON memo.id = memo_relation.memo_id WHERE memo_relation.related_memo_id = ? AND memo_relation.type = 'REFERENCE'")
        .bind(&[js_num(memo.id)])?
        .all()
        .await?;
    let outgoing: Vec<DbMemoRelation> = refs.results()?;
    let incoming: Vec<DbMemoRelation> = back_refs.results()?;
    let relations: Vec<Value> = outgoing.into_iter()
        .map(|rel| json!({ "memo": format!("memos/{}", rel.uid), "type": rel.relation_type, "direction": "outgoing", "content": rel.content }))
        .chain(incoming.into_iter().map(|rel| json!({ "memo": format!("memos/{}", rel.uid), "type": rel.relation_type, "direction": "incoming", "content": rel.content })))
        .collect();
    json_response(json!({ "relations": relations }), 200).map_err(AppError::from)
}

pub(crate) async fn set_relations(
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
    db(env)?
        .prepare("DELETE FROM memo_relation WHERE memo_id = ? AND type = 'REFERENCE'")
        .bind(&[js_num(memo.id)])?
        .run()
        .await?;
    if let Some(relations) = body.get("relations").and_then(Value::as_array) {
        for rel in relations {
            let related_uid = rel
                .get("memo")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .trim_start_matches("memos/");
            if related_uid.is_empty() || related_uid == uid {
                continue;
            }
            if let Some(related) = get_memo_by_uid(env, related_uid).await? {
                db(env)?.prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'REFERENCE')")
                    .bind(&[js_num(memo.id), js_num(related.id)])?
                    .run()
                    .await?;
            }
        }
    }
    emit_memo_change(
        env,
        "memo.updated",
        &memo,
        json!({ "relationsUpdated": true }),
    )
    .await;
    get_relations(env, viewer, uid).await
}

pub(crate) async fn suggest_memo_relations(
    env: &Env,
    viewer: &Viewer,
    uid: &str,
) -> std::result::Result<Response, AppError> {
    pub(crate) const RECENT_CANDIDATE_LIMIT: i64 = 80;
    pub(crate) const AI_CANDIDATE_LIMIT: usize = 30;

    let memo = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }

    let rows = db(env)?.prepare("SELECT memo.uid, memo.content, memo.payload, memo.updated_ts FROM memo WHERE memo.row_status = 'NORMAL' AND memo.id != ? AND (memo.visibility != 'PRIVATE' OR memo.creator_id = ? OR ? = 'ADMIN') AND NOT EXISTS (SELECT 1 FROM memo_relation WHERE memo_relation.memo_id = ? AND memo_relation.related_memo_id = memo.id AND memo_relation.type = 'REFERENCE') ORDER BY memo.updated_ts DESC, memo.id DESC LIMIT ?")
        .bind(&[js_num(memo.id), js_num(viewer.id), viewer.role.clone().into(), js_num(memo.id), js_num(RECENT_CANDIDATE_LIMIT)])?
        .all()
        .await?;
    let candidates: Vec<RelationCandidate> = rows.results()?;
    let ranked = rank_relation_candidates(
        &relation_candidate_from_memo(&memo),
        &candidates,
        AI_CANDIDATE_LIMIT,
    );
    if ranked.is_empty() {
        return json_response(json!({ "suggestions": [] }), 200).map_err(AppError::from);
    }

    let candidate_content: HashMap<String, String> = ranked
        .iter()
        .map(|candidate| (candidate.uid.clone(), candidate.content.clone()))
        .collect();
    let settings = resolve_ai_settings(env).await?;
    let ai_suggestions = if settings.api_key.trim().is_empty() {
        Vec::new()
    } else {
        request_ai_relation_suggestions(&settings, &memo, &ranked, &candidate_content)
            .await
            .unwrap_or_default()
    };
    let suggestions = if ai_suggestions.is_empty() {
        ranked
            .iter()
            .take(5)
            .map(|candidate| {
                json!({
                    "memo": format!("memos/{}", candidate.uid),
                    "content": candidate.content,
                    "reason": "标签或关键词相近",
                    "confidence": (candidate.score / 10.0).clamp(0.35, 0.75),
                    "source": "local"
                })
            })
            .collect()
    } else {
        ai_suggestions
    };

    json_response(json!({ "suggestions": suggestions }), 200).map_err(AppError::from)
}
