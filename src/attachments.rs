use super::*;

pub(crate) async fn list_attachments(
    env: &Env,
    url: &Url,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let unattached = url
        .query_pairs()
        .any(|(key, value)| key == "unattached" && value == "true");
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

pub(crate) async fn upload_attachment(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let form = req
        .form_data()
        .await
        .map_err(|_| AppError::new(400, "Invalid form data"))?;
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
        let memo = get_memo_by_uid(env, memo_uid.trim())
            .await?
            .ok_or_else(|| AppError::new(404, "Memo not found"))?;
        if !can_write(&memo, viewer) {
            return Err(AppError::new(403, "Forbidden"));
        }
        Some(memo.id)
    };

    let original_filename = file.name();
    let filename = sanitize_filename(if original_filename.is_empty() {
        "attachment"
    } else {
        &original_filename
    });
    let file_type = if file.type_().is_empty() {
        "application/octet-stream".to_string()
    } else {
        file.type_()
    };
    let uid = generate_uid("a");
    let key = attachment_storage_key(viewer.id, &uid, &filename);
    let bytes = file.bytes().await?;
    let size = bytes.len() as i64;
    let mut metadata = HashMap::new();
    metadata.insert("creatorId".to_string(), viewer.id.to_string());
    metadata.insert(
        "originalFilename".to_string(),
        if original_filename.is_empty() {
            filename.clone()
        } else {
            original_filename
        },
    );

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

    let attachment = get_attachment_by_uid(env, &uid)
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to create attachment"))?;
    json_response(json!({ "attachment": public_attachment(attachment) }), 201)
        .map_err(AppError::from)
}

pub(crate) async fn download_attachment(
    env: &Env,
    viewer: &Viewer,
    uid: &str,
) -> std::result::Result<Response, AppError> {
    let attachment = get_attachment_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Attachment not found"))?;
    if !can_read_attachment(&attachment, viewer) {
        return Err(AppError::new(403, "Forbidden"));
    }
    let object = env
        .bucket("MEMOS_BUCKET")?
        .get(attachment.reference.clone())
        .execute()
        .await?
        .ok_or_else(|| AppError::new(404, "File not found"))?;
    let body = object
        .body()
        .ok_or_else(|| AppError::new(404, "File not found"))?
        .response_body()?;
    let mut response = ResponseBuilder::new().body(body);
    response.headers_mut().set(
        "Content-Type",
        if attachment.file_type.is_empty() {
            "application/octet-stream"
        } else {
            &attachment.file_type
        },
    )?;
    response.headers_mut().set(
        "Content-Disposition",
        &format!("inline; filename=\"{}\"", attachment.filename),
    )?;
    response.headers_mut().set(
        "Cache-Control",
        if attachment.memo_visibility.as_deref() == Some("PUBLIC") {
            "public, max-age=3600"
        } else {
            "private, no-store"
        },
    )?;
    Ok(response)
}

pub(crate) async fn delete_attachment(
    env: &Env,
    viewer: &Viewer,
    uid: &str,
) -> std::result::Result<Response, AppError> {
    let attachment = get_attachment_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Attachment not found"))?;
    if viewer.role != "ADMIN" && attachment.creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    if attachment.memo_id.is_some() {
        return Err(AppError::new(
            409,
            "Only unattached attachments can be deleted",
        ));
    }
    let _ = env
        .bucket("MEMOS_BUCKET")?
        .delete(attachment.reference.clone())
        .await;
    db(env)?
        .prepare("DELETE FROM attachment WHERE id = ?")
        .bind(&[js_num(attachment.id)])?
        .run()
        .await?;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

pub(crate) async fn batch_delete_attachments(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let uids: Vec<String> = body
        .get("attachmentUids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|uid| !uid.is_empty())
                .map(ToString::to_string)
                .take(100)
                .collect()
        })
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
        let owner_sql = if viewer.role == "ADMIN" {
            ""
        } else {
            values.push(js_num(viewer.id));
            " AND attachment.creator_id = ?"
        };
        db(env)?.prepare(format!("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id IS NULL AND attachment.uid IN ({}){}", placeholders, owner_sql))
            .bind(&values)?
            .all()
            .await?
    };
    let attachments: Vec<DbAttachment> = rows.results()?;
    let mut deleted = 0;
    let mut size = 0;
    for attachment in attachments {
        let _ = env
            .bucket("MEMOS_BUCKET")?
            .delete(attachment.reference.clone())
            .await;
        db(env)?
            .prepare("DELETE FROM attachment WHERE id = ?")
            .bind(&[js_num(attachment.id)])?
            .run()
            .await?;
        deleted += 1;
        size += attachment.size;
    }
    json_response(json!({ "deleted": deleted, "size": size }), 200).map_err(AppError::from)
}

pub(crate) async fn get_attachment_by_uid(
    env: &Env,
    uid: &str,
) -> std::result::Result<Option<DbAttachment>, AppError> {
    Ok(db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.uid = ?")
        .bind(&[uid.into()])?
        .first(None)
        .await?)
}

pub(crate) fn public_attachment(attachment: DbAttachment) -> Value {
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

pub(crate) fn attachment_storage_key(creator_id: i64, uid: &str, filename: &str) -> String {
    format!("attachments/{}/{}/{}", creator_id, uid, filename)
}

pub(crate) fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_control()
            {
                '_'
            } else {
                ch
            }
        })
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

pub(crate) fn can_read_attachment(attachment: &DbAttachment, viewer: &Viewer) -> bool {
    viewer.role == "ADMIN"
        || (attachment.memo_id.is_none() && attachment.creator_id == viewer.id)
        || attachment.memo_visibility.as_deref() != Some("PRIVATE")
        || attachment.memo_creator_id == Some(viewer.id)
}
