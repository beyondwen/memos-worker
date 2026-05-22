async fn list_attachments(env: &Env, url: &Url, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let unattached = url.query_pairs().any(|(key, value)| key == "unattached" && value == "true");
    let rows = if viewer.role == "ADMIN" && unattached {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL ORDER BY attachment.created_ts DESC LIMIT 100")
            .all()
            .await?
    } else if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id ORDER BY attachment.created_ts DESC LIMIT 100")
            .all()
            .await?
    } else if unattached {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.creator_id = ? ORDER BY attachment.created_ts DESC LIMIT 100")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.creator_id = ? ORDER BY attachment.created_ts DESC LIMIT 100")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    };
    let attachments: Vec<DbAttachment> = rows.results()?;
    let payload: Vec<Value> = attachments.into_iter().map(public_attachment).collect();
    json_response(json!({ "attachments": payload }), 200).map_err(AppError::from)
}

async fn upload_attachment(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let form = req.form_data().await.map_err(|_| AppError::new(400, "Invalid form data"))?;
    let file = match form.get("file") {
        Some(FormEntry::File(file)) => file,
        _ => return Err(AppError::new(400, "file is required")),
    };
    if file.size() > 25 * 1024 * 1024 {
        return Err(AppError::new(413, "file is too large"));
    }

    let memo_uid = form.get_field("memoUid").unwrap_or_default();
    let memo_id = if memo_uid.trim().is_empty() {
        None
    } else {
        let memo = get_memo_by_uid(env, memo_uid.trim()).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
        if !can_write(&memo, viewer) {
            return Err(AppError::new(403, "Forbidden"));
        }
        Some(memo.id)
    };

    let original_filename = file.name();
    let filename = sanitize_filename(if original_filename.is_empty() { "attachment" } else { &original_filename });
    let file_type = if file.type_().is_empty() { "application/octet-stream".to_string() } else { file.type_() };
    let uid = generate_uid("a");
    let key = attachment_storage_key(viewer.id, &uid, &filename);
    let bytes = file.bytes().await?;
    let size = bytes.len() as i64;
    let mut metadata = HashMap::new();
    metadata.insert("creatorId".to_string(), viewer.id.to_string());
    metadata.insert("originalFilename".to_string(), if original_filename.is_empty() { filename.clone() } else { original_filename });

    env.bucket("MEMOS_BUCKET")?
        .put(key.clone(), bytes)
        .http_metadata(HttpMetadata {
            content_type: Some(file_type.clone()),
            content_disposition: Some(format!("inline; filename=\"{}\"", filename)),
            ..Default::default()
        })
        .custom_metadata(metadata)
        .execute()
        .await?;

    let now = unix_now();
    db(env)?.prepare("INSERT INTO attachment (uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'S3', ?, '{}')")
        .bind(&[
            uid.clone().into(),
            js_num(viewer.id),
            js_num(now),
            js_num(now),
            filename.into(),
            file_type.into(),
            js_num(size),
            memo_id.map(js_num).unwrap_or(JsValue::NULL),
            key.into(),
        ])?
        .run()
        .await?;

    let attachment = get_attachment_by_uid(env, &uid).await?.ok_or_else(|| AppError::new(500, "Failed to create attachment"))?;
    json_response(json!({ "attachment": public_attachment(attachment) }), 201).map_err(AppError::from)
}

async fn download_attachment(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let attachment = get_attachment_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Attachment not found"))?;
    if !can_read_attachment(&attachment, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let object = env.bucket("MEMOS_BUCKET")?.get(attachment.reference.clone()).execute().await?
        .ok_or_else(|| AppError::new(404, "File not found"))?;
    let body = object.body().ok_or_else(|| AppError::new(404, "File not found"))?.response_body()?;
    let mut response = ResponseBuilder::new().body(body);
    response.headers_mut().set("Content-Type", if attachment.file_type.is_empty() { "application/octet-stream" } else { &attachment.file_type })?;
    response.headers_mut().set("Content-Disposition", &format!("inline; filename=\"{}\"", attachment.filename))?;
    response.headers_mut().set("Cache-Control", if attachment.memo_visibility.as_deref() == Some("PUBLIC") { "public, max-age=3600" } else { "private, no-store" })?;
    Ok(response)
}

async fn delete_attachment(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let attachment = get_attachment_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Attachment not found"))?;
    if viewer.role != "ADMIN" && attachment.creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    if attachment.memo_id.is_some() {
        return Err(AppError::new(409, "Only unattached attachments can be deleted"));
    }
    let _ = env.bucket("MEMOS_BUCKET")?.delete(attachment.reference.clone()).await;
    db(env)?.prepare("DELETE FROM attachment WHERE id = ?")
        .bind(&[js_num(attachment.id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

async fn batch_delete_attachments(req: &mut Request, env: &Env, viewer: &Viewer) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let uids: Vec<String> = body.get("attachmentUids")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::trim).filter(|uid| !uid.is_empty()).map(ToString::to_string).take(100).collect())
        .unwrap_or_default();
    let rows = if uids.is_empty() && viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL LIMIT 100")
            .all()
            .await?
    } else if uids.is_empty() {
        db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.creator_id = ? LIMIT 100")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    } else {
        let placeholders = placeholders(uids.len());
        let mut values: Vec<JsValue> = uids.iter().map(|uid| uid.clone().into()).collect();
        let owner_sql = if viewer.role == "ADMIN" { "" } else { values.push(js_num(viewer.id)); " AND attachment.creator_id = ?" };
        db(env)?.prepare(format!("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.uid IN ({}){}", placeholders, owner_sql))
            .bind(&values)?
            .all()
            .await?
    };
    let attachments: Vec<DbAttachment> = rows.results()?;
    let mut deleted = 0;
    let mut size = 0;
    for attachment in attachments {
        let _ = env.bucket("MEMOS_BUCKET")?.delete(attachment.reference.clone()).await;
        db(env)?.prepare("DELETE FROM attachment WHERE id = ?")
            .bind(&[js_num(attachment.id)])?
            .run()
            .await?;
        deleted += 1;
        size += attachment.size;
    }
    json_response(json!({ "deleted": deleted, "size": size }), 200).map_err(AppError::from)
}

async fn get_attachment_by_uid(env: &Env, uid: &str) -> std::result::Result<Option<DbAttachment>, AppError> {
    Ok(db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.uid = ?")
        .bind(&[uid.into()])?
        .first(None)
        .await?)
}

fn public_attachment(attachment: DbAttachment) -> Value {
    json!({
        "name": format!("attachments/{}", attachment.uid),
        "uid": attachment.uid,
        "filename": attachment.filename,
        "type": attachment.file_type,
        "size": attachment.size,
        "memoId": attachment.memo_id,
        "createdTs": attachment.created_ts,
        "url": format!("/file/attachments/{}/{}", attachment.uid, attachment.filename)
    })
}

fn attachment_storage_key(creator_id: i64, uid: &str, filename: &str) -> String {
    format!("attachments/{}/{}/{}", creator_id, uid, filename)
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| if matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_control() { '_' } else { ch })
        .collect::<String>()
        .trim()
        .chars()
        .take(180)
        .collect();
    if cleaned.is_empty() || !cleaned.chars().any(char::is_alphanumeric) {
        "attachment".to_string()
    } else {
        cleaned
    }
}

fn can_read_attachment(attachment: &DbAttachment, viewer: &Viewer) -> bool {
    viewer.role == "ADMIN"
        || (attachment.memo_id.is_none() && attachment.creator_id == viewer.id)
        || attachment.memo_visibility.as_deref() != Some("PRIVATE")
        || attachment.memo_creator_id == Some(viewer.id)
}

async fn list_comments(env: &Env, viewer: &Viewer, parent_uid: &str) -> std::result::Result<Response, AppError> {
    let parent = get_memo_by_uid(env, parent_uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
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

async fn create_comment(req: &mut Request, env: &Env, viewer: &Viewer, parent_uid: &str) -> std::result::Result<Response, AppError> {
    let parent = get_memo_by_uid(env, parent_uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&parent, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let content = body.get("content").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Err(AppError::new(400, "Content is required"));
    }
    let uid = generate_uid("m");
    let now = unix_now();
    db(env)?.prepare("INSERT INTO memo (uid, creator_id, created_ts, updated_ts, content, visibility, payload) VALUES (?, ?, ?, ?, ?, 'PROTECTED', ?)")
        .bind(&[uid.clone().into(), js_num(viewer.id), js_num(now), js_num(now), content.into(), build_memo_payload("").to_string().into()])?
        .run()
        .await?;
    let comment = get_memo_by_uid(env, &uid).await?.ok_or_else(|| AppError::new(500, "Failed to create comment"))?;
    db(env)?.prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'COMMENT')")
        .bind(&[js_num(parent.id), js_num(comment.id)])?
        .run()
        .await?;
    if let Err(error) = record_comment_inbox(env, viewer.id, parent.creator_id, &parent.uid, &comment.uid).await {
        console_log!("comment inbox record failed: {}", error.message);
    }
    emit_memo_change(env, "memo.created", &comment, json!({ "parentMemoUid": parent.uid.clone() })).await;
    emit_memo_change(env, "memo.comment.created", &parent, json!({ "comment": public_memo(comment.clone()) })).await;
    let comment = memo_with_attachments(env, comment).await?;
    json_response(json!({ "memo": comment }), 201).map_err(AppError::from)
}

async fn list_reactions(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    list_reactions_for_memo(env, memo.id).await
}

async fn upsert_reaction(req: &mut Request, env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let reaction_type = body.get("reactionType").and_then(Value::as_str).unwrap_or("").trim();
    if reaction_type.is_empty() {
        return Err(AppError::new(400, "reactionType is required"));
    }
    db(env)?.prepare("INSERT INTO reaction (created_ts, creator_id, content_type, content_id, reaction_type) VALUES (?, ?, 'MEMO', ?, ?) ON CONFLICT (creator_id, content_type, content_id, reaction_type) DO NOTHING")
        .bind(&[js_num(unix_now()), js_num(viewer.id), js_num(memo.id), reaction_type.into()])?
        .run()
        .await?;
    emit_memo_change(env, "reaction.upserted", &memo, json!({ "reactionType": reaction_type, "actorId": viewer.id })).await;
    list_reactions_for_memo(env, memo.id).await
}

async fn delete_reaction(env: &Env, viewer: &Viewer, uid: &str, reaction_id: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    let id = reaction_id.parse::<i64>().map_err(|_| AppError::new(400, "Invalid reaction ID"))?;
    let row: Option<Value> = db(env)?.prepare("SELECT id, creator_id FROM reaction WHERE id = ? AND content_type = 'MEMO' AND content_id = ?")
        .bind(&[js_num(id), js_num(memo.id)])?
        .first(None)
        .await?;
    let row = row.ok_or_else(|| AppError::new(404, "Reaction not found"))?;
    let creator_id = row.get("creator_id").and_then(Value::as_i64).unwrap_or_default();
    if viewer.role != "ADMIN" && creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    db(env)?.prepare("DELETE FROM reaction WHERE id = ?")
        .bind(&[js_num(id)])?
        .run()
        .await?;
    emit_memo_change(env, "reaction.deleted", &memo, json!({ "reactionId": id, "actorId": viewer.id })).await;
    list_reactions_for_memo(env, memo.id).await
}

async fn list_reactions_for_memo(env: &Env, memo_id: i64) -> std::result::Result<Response, AppError> {
    let rows = db(env)?.prepare("SELECT reaction.id, reaction.created_ts, reaction.reaction_type, reaction.creator_id, \"user\".username AS creator_username FROM reaction JOIN \"user\" ON \"user\".id = reaction.creator_id WHERE reaction.content_type = 'MEMO' AND reaction.content_id = ? ORDER BY reaction.created_ts ASC")
        .bind(&[js_num(memo_id)])?
        .all()
        .await?;
    let reactions: Vec<DbReaction> = rows.results()?;
    let payload: Vec<Value> = reactions.into_iter().map(|reaction| json!({
        "id": reaction.id,
        "reactionType": reaction.reaction_type,
        "creator": { "id": reaction.creator_id, "username": reaction.creator_username },
        "createdTs": reaction.created_ts
    })).collect();
    json_response(json!({ "reactions": payload }), 200).map_err(AppError::from)
}

async fn get_relations(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
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

async fn set_relations(req: &mut Request, env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_write(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let body: Value = req.json().await.map_err(|_| AppError::new(400, "Invalid JSON"))?;
    db(env)?.prepare("DELETE FROM memo_relation WHERE memo_id = ? AND type = 'REFERENCE'")
        .bind(&[js_num(memo.id)])?
        .run()
        .await?;
    if let Some(relations) = body.get("relations").and_then(Value::as_array) {
        for rel in relations {
            let related_uid = rel.get("memo").and_then(Value::as_str).unwrap_or("").trim().trim_start_matches("memos/");
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
    emit_memo_change(env, "memo.updated", &memo, json!({ "relationsUpdated": true })).await;
    get_relations(env, viewer, uid).await
}

async fn suggest_memo_relations(env: &Env, viewer: &Viewer, uid: &str) -> std::result::Result<Response, AppError> {
    const RECENT_CANDIDATE_LIMIT: i64 = 80;
    const AI_CANDIDATE_LIMIT: usize = 30;

    let memo = get_memo_by_uid(env, uid).await?.ok_or_else(|| AppError::new(404, "Memo not found"))?;
    if !can_read(&memo, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }

    let rows = db(env)?.prepare("SELECT memo.uid, memo.content, memo.payload, memo.updated_ts FROM memo WHERE memo.row_status = 'NORMAL' AND memo.id != ? AND (memo.visibility != 'PRIVATE' OR memo.creator_id = ? OR ? = 'ADMIN') AND NOT EXISTS (SELECT 1 FROM memo_relation WHERE memo_relation.memo_id = ? AND memo_relation.related_memo_id = memo.id AND memo_relation.type = 'REFERENCE') ORDER BY memo.updated_ts DESC, memo.id DESC LIMIT ?")
        .bind(&[js_num(memo.id), js_num(viewer.id), viewer.role.clone().into(), js_num(memo.id), js_num(RECENT_CANDIDATE_LIMIT)])?
        .all()
        .await?;
    let candidates: Vec<RelationCandidate> = rows.results()?;
    let ranked = rank_relation_candidates(&relation_candidate_from_memo(&memo), &candidates, AI_CANDIDATE_LIMIT);
    if ranked.is_empty() {
        return json_response(json!({ "suggestions": [] }), 200).map_err(AppError::from);
    }

    let candidate_content: HashMap<String, String> = ranked.iter()
        .map(|candidate| (candidate.uid.clone(), candidate.content.clone()))
        .collect();
    let settings = resolve_ai_settings(env).await?;
    let ai_suggestions = if settings.api_key.trim().is_empty() {
        Vec::new()
    } else {
        request_ai_relation_suggestions(&settings, &memo, &ranked, &candidate_content).await.unwrap_or_default()
    };
    let suggestions = if ai_suggestions.is_empty() {
        ranked.iter().take(5).map(|candidate| json!({
            "memo": format!("memos/{}", candidate.uid),
            "content": candidate.content,
            "reason": "标签或关键词相近",
            "confidence": (candidate.score / 10.0).clamp(0.35, 0.75),
            "source": "local"
        })).collect()
    } else {
        ai_suggestions
    };

    json_response(json!({ "suggestions": suggestions }), 200).map_err(AppError::from)
}

async fn request_ai_relation_suggestions(
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
    let request = Request::new_with_init(&format!("{}/chat/completions", settings.base_url.trim_end_matches('/')), &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(502, format!("AI API returned HTTP {}", response.status_code())));
    }
    let data: Value = response.json().await?;
    let content = data.get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    Ok(parse_ai_relation_suggestions(content, candidate_content))
}

fn relation_candidate_from_memo(memo: &DbMemo) -> RelationCandidate {
    RelationCandidate {
        uid: memo.uid.clone(),
        content: memo.content.clone(),
        payload: memo.payload.clone(),
        updated_ts: memo.updated_ts,
    }
}

fn rank_relation_candidates(current: &RelationCandidate, candidates: &[RelationCandidate], limit: usize) -> Vec<RankedRelationCandidate> {
    let current_tags = extract_payload_tags(&current.payload);
    let current_keywords = extract_keywords(&current.content);
    let max_updated = candidates.iter().map(|candidate| candidate.updated_ts).chain(std::iter::once(current.updated_ts)).max().unwrap_or(1).max(1);
    let mut ranked: Vec<RankedRelationCandidate> = candidates.iter()
        .filter(|candidate| candidate.uid != current.uid && !candidate.content.trim().is_empty())
        .filter_map(|candidate| {
            let tags = extract_payload_tags(&candidate.payload);
            let keywords = extract_keywords(&candidate.content);
            let shared_tags = tags.iter().filter(|tag| current_tags.contains(*tag)).count() as f64;
            let shared_keywords = keywords.iter().filter(|keyword| current_keywords.contains(*keyword)).count() as f64;
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
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(limit);
    ranked
}

fn parse_ai_relation_suggestions(raw: &str, candidate_content: &HashMap<String, String>) -> Vec<Value> {
    const SUGGESTION_LIMIT: usize = 8;

    let parsed = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!({}));
    let list = parsed.get("suggestions").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut seen = BTreeSet::new();
    let mut suggestions = Vec::new();
    for item in list {
        let uid = item.get("memo")
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
        let reason = truncate(item.get("reason").and_then(Value::as_str).unwrap_or("内容相关"), 160);
        let confidence = item.get("confidence").and_then(Value::as_f64).map(clamp_confidence).unwrap_or(0.5);
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

fn extract_payload_tags(payload: &str) -> Vec<String> {
    let parsed = serde_json::from_str::<Value>(payload).unwrap_or_else(|_| json!({}));
    parsed.get("tags")
        .and_then(Value::as_array)
        .map(|tags| tags.iter().filter_map(Value::as_str).map(str::trim).filter(|tag| !tag.is_empty()).map(ToString::to_string).collect())
        .unwrap_or_default()
}

fn extract_keywords(content: &str) -> Vec<String> {
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

fn push_keyword(words: &mut BTreeSet<String>, current: &mut String) {
    let len = current.chars().count();
    if (2..=32).contains(&len) {
        words.insert(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn clamp_confidence(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}
