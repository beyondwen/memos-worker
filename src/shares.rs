use super::*;

pub(crate) async fn create_share(
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
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let expires_ts = body.get("expiresTs").and_then(Value::as_i64);
    let share_uid = generate_uid("s")?;
    let now = unix_now();
    db(env)?.prepare("INSERT INTO memo_share (uid, memo_id, creator_id, created_ts, expires_ts) VALUES (?, ?, ?, ?, ?)")
        .bind(&[
            share_uid.clone().into(),
            js_num(memo.id),
            js_num(viewer.id),
            js_num(now),
            expires_ts.map(js_num).unwrap_or(JsValue::NULL),
        ])?
        .run()
        .await?;
    emit_memo_change(
        env,
        "share.created",
        &memo,
        json!({ "shareUid": share_uid.clone(), "expiresTs": expires_ts }),
    )
    .await;
    json_response(
        json!({
            "share": {
                "uid": share_uid,
                "memoUid": memo.uid,
                "createdTs": now,
                "expiresTs": expires_ts,
                "url": format!("/api/v1/shares/{}", share_uid)
            }
        }),
        201,
    )
    .map_err(AppError::from)
}

pub(crate) async fn list_shares(
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
    let rows = db(env)?
        .prepare("SELECT * FROM memo_share WHERE memo_id = ? ORDER BY created_ts DESC")
        .bind(&[js_num(memo.id)])?
        .all()
        .await?;
    let shares: Vec<DbShare> = rows.results()?;
    let payload: Vec<Value> = shares
        .into_iter()
        .map(|share| {
            json!({
                "id": share.id,
                "uid": share.uid,
                "memoUid": memo.uid,
                "createdTs": share.created_ts,
                "expiresTs": share.expires_ts,
                "url": format!("/api/v1/shares/{}", share.uid)
            })
        })
        .collect();
    json_response(json!({ "shares": payload }), 200).map_err(AppError::from)
}

pub(crate) async fn delete_share(
    env: &Env,
    viewer: &Viewer,
    uid: &str,
    share_id: &str,
) -> std::result::Result<Response, AppError> {
    let memo = get_memo_by_uid(env, uid)
        .await?
        .ok_or_else(|| AppError::new(404, "Memo not found"))?;
    let id = share_id
        .parse::<i64>()
        .map_err(|_| AppError::new(400, "Invalid share ID"))?;
    let share: Option<DbShare> = db(env)?
        .prepare("SELECT * FROM memo_share WHERE id = ? AND memo_id = ?")
        .bind(&[js_num(id), js_num(memo.id)])?
        .first(None)
        .await?;
    let share = share.ok_or_else(|| AppError::new(404, "Share not found"))?;
    if viewer.role != "ADMIN" && share.creator_id != viewer.id {
        return Err(AppError::new(403, "Forbidden"));
    }
    db(env)?
        .prepare("DELETE FROM memo_share WHERE id = ?")
        .bind(&[js_num(id)])?
        .run()
        .await?;
    emit_memo_change(
        env,
        "share.deleted",
        &memo,
        json!({ "shareId": share.id, "shareUid": share.uid }),
    )
    .await;
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}
