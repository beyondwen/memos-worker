use super::*;

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
