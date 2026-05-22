use super::*;

pub(crate) async fn get_recent_memos(
    env: &Env,
    viewer: &Viewer,
    limit: i64,
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let rows = if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id ORDER BY memo.created_ts DESC LIMIT ?")
            .bind(&[js_num(limit)])?
            .all().await?
    } else {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.visibility != 'PRIVATE' OR memo.creator_id = ? ORDER BY memo.created_ts DESC LIMIT ?")
            .bind(&[js_num(viewer.id), js_num(limit)])?
            .all().await?
    };
    Ok(rows.results()?)
}

pub(crate) async fn get_memos_by_uids(
    env: &Env,
    viewer: &Viewer,
    uids: &[String],
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let placeholders = placeholders(uids.len());
    let mut values: Vec<JsValue> = uids.iter().map(|uid| uid.clone().into()).collect();
    let sql = if viewer.role == "ADMIN" {
        format!("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.uid IN ({})", placeholders)
    } else {
        values.push(js_num(viewer.id));
        format!("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.uid IN ({}) AND memo.creator_id = ?", placeholders)
    };
    let rows = db(env)?.prepare(sql).bind(&values)?.all().await?;
    Ok(rows.results()?)
}

pub(crate) async fn get_memo_by_uid(
    env: &Env,
    uid: &str,
) -> std::result::Result<Option<DbMemo>, AppError> {
    Ok(db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.uid = ?")
        .bind(&[uid.into()])?
        .first(None)
        .await?)
}

pub(crate) async fn get_user_by_username(
    env: &Env,
    username: &str,
) -> std::result::Result<Option<DbUser>, AppError> {
    Ok(db(env)?
        .prepare("SELECT * FROM \"user\" WHERE username = ?")
        .bind(&[username.into()])?
        .first(None)
        .await?)
}

pub(crate) async fn get_user_by_id(
    env: &Env,
    id: i64,
) -> std::result::Result<Option<DbUser>, AppError> {
    Ok(db(env)?
        .prepare("SELECT * FROM \"user\" WHERE id = ?")
        .bind(&[js_num(id)])?
        .first(None)
        .await?)
}

pub(crate) async fn resolve_user(
    env: &Env,
    identifier: &str,
) -> std::result::Result<Option<DbUser>, AppError> {
    let decoded = identifier.trim();
    if let Ok(id) = decoded.parse::<i64>() {
        return get_user_by_id(env, id).await;
    }
    get_user_by_username(env, decoded).await
}

pub(crate) async fn purge_ids(env: &Env, ids: &[i64]) -> std::result::Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = placeholders(ids.len());
    let values: Vec<JsValue> = ids.iter().map(|id| js_num(*id)).collect();
    db(env)?
        .prepare(format!(
            "UPDATE attachment SET memo_id = NULL, updated_ts = ? WHERE memo_id IN ({})",
            placeholders
        ))
        .bind(&bind_with_first(unix_now(), ids))?
        .run()
        .await?;
    let mut relation_values = values.clone();
    relation_values.extend(values.clone());
    db(env)?
        .prepare(format!(
            "DELETE FROM memo_relation WHERE memo_id IN ({}) OR related_memo_id IN ({})",
            placeholders, placeholders
        ))
        .bind(&relation_values)?
        .run()
        .await?;
    db(env)?
        .prepare(format!("DELETE FROM memo WHERE id IN ({})", placeholders))
        .bind(&values)?
        .run()
        .await?;
    Ok(())
}

pub(crate) fn public_user(user: DbUser) -> PublicUser {
    PublicUser {
        id: user.id,
        username: user.username,
        role: user.role,
        nickname: user.nickname,
        email: user.email,
        avatar_url: user.avatar_url,
        description: user.description,
    }
}

pub(crate) fn public_memo(memo: DbMemo) -> PublicMemo {
    public_memo_with_attachments(memo, vec![])
}

pub(crate) fn public_memo_with_attachments(memo: DbMemo, attachments: Vec<Value>) -> PublicMemo {
    PublicMemo {
        name: format!("memos/{}", memo.uid),
        id: memo.id,
        uid: memo.uid,
        creator: MemoCreator {
            id: memo.creator_id,
            username: memo.creator_username,
            nickname: memo.creator_nickname,
        },
        created_ts: memo.created_ts,
        updated_ts: memo.updated_ts,
        row_status: memo.row_status,
        content: memo.content,
        visibility: memo.visibility,
        pinned: memo.pinned != 0,
        payload: serde_json::from_str(&memo.payload).unwrap_or_else(|_| json!({})),
        attachments,
    }
}

pub(crate) async fn memo_with_attachments(
    env: &Env,
    memo: DbMemo,
) -> std::result::Result<PublicMemo, AppError> {
    let attachments = list_attachments_for_memo(env, memo.id).await?;
    Ok(public_memo_with_attachments(memo, attachments))
}

pub(crate) async fn list_attachments_for_memo(
    env: &Env,
    memo_id: i64,
) -> std::result::Result<Vec<Value>, AppError> {
    let rows = db(env)?.prepare("SELECT attachment.*, memo.visibility AS memo_visibility, memo.creator_id AS memo_creator_id FROM attachment LEFT JOIN memo ON memo.id = attachment.memo_id WHERE attachment.memo_id = ? ORDER BY attachment.created_ts, attachment.id")
        .bind(&[js_num(memo_id)])?
        .all()
        .await?;
    let attachments: Vec<DbAttachment> = rows.results()?;
    Ok(attachments.into_iter().map(public_attachment).collect())
}

pub(crate) fn can_read(memo: &DbMemo, viewer: &Viewer) -> bool {
    viewer.role == "ADMIN" || memo.visibility != "PRIVATE" || memo.creator_id == viewer.id
}

pub(crate) fn can_write(memo: &DbMemo, viewer: &Viewer) -> bool {
    viewer.role == "ADMIN" || memo.creator_id == viewer.id
}

pub(crate) fn require_admin(viewer: &Viewer) -> std::result::Result<(), AppError> {
    if viewer.role == "ADMIN" {
        Ok(())
    } else {
        Err(AppError::new(403, "Forbidden"))
    }
}
