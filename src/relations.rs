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

pub(crate) async fn request_ai_relation_suggestions(
    settings: &AiSettings,
    memo: &DbMemo,
    candidates: &[RankedRelationCandidate],
    candidate_content: &HashMap<String, String>,
) -> std::result::Result<Vec<Value>, AppError> {
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", settings.api_key))?;
    headers.set("Content-Type", "application/json")?;
    let payload = json!({
        "model": settings.model,
        "temperature": 0.1,
        "response_format": { "type": "json_object" },
        "messages": [
            {
                "role": "system",
                "content": "你是个人知识库的关联识别助手。只返回 JSON，不要解释。"
            },
            {
                "role": "user",
                "content": json!({
                    "task": "从 candidates 中选择最多 8 条和 currentMemo 最相关的笔记。返回 {\"suggestions\":[{\"memo\":\"memos/<uid>\",\"reason\":\"简短原因\",\"confidence\":0.0到1.0}]}。",
                    "currentMemo": {
                        "memo": format!("memos/{}", memo.uid),
                        "content": truncate(&memo.content, 1200)
                    },
                    "candidates": candidates.iter().map(|candidate| json!({
                        "memo": format!("memos/{}", candidate.uid),
                        "content": truncate(&candidate.content, 600),
                        "tags": candidate.tags
                    })).collect::<Vec<_>>()
                }).to_string()
            }
        ]
    });
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&payload.to_string())));
    let request = Request::new_with_init(
        &format!(
            "{}/chat/completions",
            settings.base_url.trim_end_matches('/')
        ),
        &init,
    )?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(
            502,
            format!("AI API returned HTTP {}", response.status_code()),
        ));
    }
    let data: Value = response.json().await?;
    let content = data
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(parse_ai_relation_suggestions(content, candidate_content))
}

pub(crate) fn relation_candidate_from_memo(memo: &DbMemo) -> RelationCandidate {
    RelationCandidate {
        uid: memo.uid.clone(),
        content: memo.content.clone(),
        payload: memo.payload.clone(),
        updated_ts: memo.updated_ts,
    }
}

pub(crate) fn rank_relation_candidates(
    current: &RelationCandidate,
    candidates: &[RelationCandidate],
    limit: usize,
) -> Vec<RankedRelationCandidate> {
    let current_tags = extract_payload_tags(&current.payload);
    let current_keywords = extract_keywords(&current.content);
    let max_updated = candidates
        .iter()
        .map(|candidate| candidate.updated_ts)
        .chain(std::iter::once(current.updated_ts))
        .max()
        .unwrap_or(1)
        .max(1);
    let mut ranked: Vec<RankedRelationCandidate> = candidates
        .iter()
        .filter(|candidate| candidate.uid != current.uid && !candidate.content.trim().is_empty())
        .filter_map(|candidate| {
            let tags = extract_payload_tags(&candidate.payload);
            let keywords = extract_keywords(&candidate.content);
            let shared_tags = tags
                .iter()
                .filter(|tag| current_tags.contains(*tag))
                .count() as f64;
            let shared_keywords = keywords
                .iter()
                .filter(|keyword| current_keywords.contains(*keyword))
                .count() as f64;
            let recency = (candidate.updated_ts as f64 / max_updated as f64).clamp(0.0, 1.0);
            let score = shared_tags * 5.0 + shared_keywords * 2.0 + recency;
            if score > 0.0 {
                Some(RankedRelationCandidate {
                    uid: candidate.uid.clone(),
                    content: candidate.content.clone(),
                    score,
                    tags,
                })
            } else {
                None
            }
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.truncate(limit);
    ranked
}

pub(crate) fn parse_ai_relation_suggestions(
    raw: &str,
    candidate_content: &HashMap<String, String>,
) -> Vec<Value> {
    pub(crate) const SUGGESTION_LIMIT: usize = 8;

    let parsed = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!({}));
    let list = parsed
        .get("suggestions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = BTreeSet::new();
    let mut suggestions = Vec::new();
    for item in list {
        let uid = item
            .get("memo")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim_start_matches("memos/")
            .trim()
            .to_string();
        let Some(content) = candidate_content.get(&uid) else {
            continue;
        };
        if uid.is_empty() || !seen.insert(uid.clone()) {
            continue;
        }
        let reason = truncate(
            item.get("reason")
                .and_then(Value::as_str)
                .unwrap_or("内容相关"),
            160,
        );
        let confidence = item
            .get("confidence")
            .and_then(Value::as_f64)
            .map(clamp_confidence)
            .unwrap_or(0.5);
        suggestions.push(json!({
            "memo": format!("memos/{}", uid),
            "content": content,
            "reason": reason,
            "confidence": confidence,
            "source": "ai"
        }));
        if suggestions.len() >= SUGGESTION_LIMIT {
            break;
        }
    }
    suggestions
}

pub(crate) fn extract_payload_tags(payload: &str) -> Vec<String> {
    let parsed = serde_json::from_str::<Value>(payload).unwrap_or_else(|_| json!({}));
    parsed
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn extract_keywords(content: &str) -> Vec<String> {
    let mut words = BTreeSet::new();
    let mut current = String::new();
    for ch in content.to_lowercase().chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            current.push(ch);
        } else {
            push_keyword(&mut words, &mut current);
        }
    }
    push_keyword(&mut words, &mut current);
    words.into_iter().take(80).collect()
}

pub(crate) fn push_keyword(words: &mut BTreeSet<String>, current: &mut String) {
    let len = current.chars().count();
    if (2..=32).contains(&len) {
        words.insert(std::mem::take(current));
    } else {
        current.clear();
    }
}

pub(crate) fn clamp_confidence(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}
