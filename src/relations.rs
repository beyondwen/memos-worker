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
    let requested_uids = requested_relation_uids(&body, uid);
    let mut related_ids = Vec::with_capacity(requested_uids.len());
    for related_uid in requested_uids {
        let related = get_memo_by_uid(env, &related_uid).await?.ok_or_else(|| {
            AppError::new(400, format!("Related memo not found: {}", related_uid))
        })?;
        if !can_read(&related, viewer) {
            return Err(AppError::new(403, "Forbidden related memo"));
        }
        related_ids.push(related.id);
    }

    let database = db(env)?;
    let mut statements = vec![database
        .prepare("DELETE FROM memo_relation WHERE memo_id = ? AND type = 'REFERENCE'")
        .bind(&[js_num(memo.id)])?];
    for related_id in related_ids {
        statements.push(database.prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'REFERENCE')")
            .bind(&[js_num(memo.id), js_num(related_id)])?);
    }
    database.batch(statements).await?;

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
    pub(crate) const CANDIDATE_SCAN_LIMIT: i64 = 2500;
    pub(crate) const AI_CANDIDATE_LIMIT: usize = 45;

    let memo = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }

    let rows = db(env)?.prepare("SELECT memo.uid, memo.content, memo.payload, memo.updated_ts FROM memo WHERE memo.row_status = 'NORMAL' AND memo.id != ? AND (memo.visibility != 'PRIVATE' OR memo.creator_id = ? OR ? = 'ADMIN') AND NOT EXISTS (SELECT 1 FROM memo_relation WHERE memo_relation.memo_id = ? AND memo_relation.related_memo_id = memo.id AND memo_relation.type = 'REFERENCE') ORDER BY memo.updated_ts DESC, memo.id DESC LIMIT ?")
        .bind(&[js_num(memo.id), js_num(viewer.id), viewer.role.clone().into(), js_num(memo.id), js_num(CANDIDATE_SCAN_LIMIT)])?
        .all()
        .await?;
    let candidates: Vec<RelationCandidate> = rows.results()?;
    let ranked = rank_relation_candidates(
        &relation_candidate_from_memo(&memo),
        &candidates,
        AI_CANDIDATE_LIMIT,
    );
    if ranked.is_empty() {
        return json_response(
            relation_suggestions_payload(Vec::new(), RelationSuggestionSource::Local),
            200,
        )
        .map_err(AppError::from);
    }

    let candidate_content: HashMap<String, String> = ranked
        .iter()
        .map(|candidate| (candidate.uid.clone(), candidate.content.clone()))
        .collect();
    let settings = resolve_ai_settings(env).await?;
    let (suggestions, source) = if settings.api_key.trim().is_empty() {
        (
            local_relation_suggestions(&ranked),
            RelationSuggestionSource::Local,
        )
    } else {
        match request_ai_relation_suggestions(&settings, &memo, &ranked, &candidate_content).await {
            Ok(ai_suggestions) if !ai_suggestions.is_empty() => {
                (ai_suggestions, RelationSuggestionSource::Ai)
            }
            Ok(_) => (
                local_relation_suggestions(&ranked),
                RelationSuggestionSource::Local,
            ),
            Err(err) => (
                local_relation_suggestions(&ranked),
                RelationSuggestionSource::LocalFallback {
                    warning: Some(err.message),
                },
            ),
        }
    };

    json_response(relation_suggestions_payload(suggestions, source), 200).map_err(AppError::from)
}

pub(crate) enum RelationSuggestionSource {
    Ai,
    Local,
    LocalFallback { warning: Option<String> },
}

pub(crate) fn relation_suggestions_payload(
    suggestions: Vec<Value>,
    source: RelationSuggestionSource,
) -> Value {
    let (source, warning) = match source {
        RelationSuggestionSource::Ai => ("ai", None),
        RelationSuggestionSource::Local => ("local", None),
        RelationSuggestionSource::LocalFallback { warning } => ("local", warning),
    };
    let mut payload = json!({
        "suggestions": suggestions,
        "source": source,
    });
    if let Some(warning) = warning.filter(|message| !message.trim().is_empty()) {
        payload["warning"] = json!(warning);
    }
    payload
}

pub(crate) fn requested_relation_uids(body: &Value, current_uid: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut uids = Vec::new();
    let Some(relations) = body.get("relations").and_then(Value::as_array) else {
        return uids;
    };
    for relation in relations {
        let uid = relation
            .get("memo")
            .and_then(Value::as_str)
            .map(relation_uid_from_ref)
            .unwrap_or_else(String::new);
        if uid.is_empty() || uid == current_uid || !seen.insert(uid.clone()) {
            continue;
        }
        uids.push(uid);
    }
    uids
}

fn relation_uid_from_ref(value: &str) -> String {
    let trimmed = value.trim();
    let uid_start = trimmed.find("/memos/").map(|index| index + "/memos/".len());
    let raw_uid = match uid_start {
        Some(index) => &trimmed[index..],
        None => trimmed.strip_prefix("memos/").unwrap_or(trimmed),
    };
    raw_uid
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '?' | '#' | ',' | '/'))
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(crate) fn local_relation_suggestions(ranked: &[RankedRelationCandidate]) -> Vec<Value> {
    ranked
        .iter()
        .take(8)
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
}
