use super::*;

pub(crate) async fn export_data(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let users = db(env)?.prepare("SELECT id, username, role, email, nickname, avatar_url, description, row_status FROM \"user\" ORDER BY id")
        .all()
        .await?;
    let memos = db(env)?
        .prepare("SELECT * FROM memo ORDER BY created_ts, id")
        .all()
        .await?;
    let attachments = db(env)?.prepare("SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload FROM attachment ORDER BY created_ts, id")
        .all()
        .await?;
    let users: Vec<Value> = users.results()?;
    let memos: Vec<Value> = memos.results()?;
    let attachments: Vec<Value> = attachments.results()?;
    json_response(
        json!({
            "exportedAt": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default(),
            "users": users,
            "memos": memos,
            "attachments": attachments
        }),
        200,
    )
    .map_err(AppError::from)
}

pub(crate) async fn import_data(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let mut imported = 0;
    let now = unix_now();
    if let Some(memos) = body.get("memos").and_then(Value::as_array) {
        for item in memos {
            let content = item
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if content.is_empty() {
                continue;
            }
            let uid = match item.get("uid").and_then(Value::as_str) {
                Some(uid) => uid.to_string(),
                None => generate_uid("m")?,
            };
            let created_ts = item
                .get("created_ts")
                .or_else(|| item.get("createdTs"))
                .and_then(Value::as_i64)
                .unwrap_or(now);
            let updated_ts = item
                .get("updated_ts")
                .or_else(|| item.get("updatedTs"))
                .and_then(Value::as_i64)
                .unwrap_or(created_ts);
            let row_status = normalize_state(
                item.get("row_status")
                    .or_else(|| item.get("rowStatus"))
                    .and_then(Value::as_str)
                    .unwrap_or("NORMAL"),
            )?;
            let visibility = normalize_visibility(
                item.get("visibility")
                    .and_then(Value::as_str)
                    .unwrap_or("PRIVATE"),
            )?;
            let pinned = item
                .get("pinned")
                .and_then(Value::as_bool)
                .map(|value| if value { 1 } else { 0 })
                .or_else(|| item.get("pinned").and_then(Value::as_i64))
                .unwrap_or(0);
            let payload = item
                .get("payload")
                .cloned()
                .unwrap_or_else(|| build_memo_payload(content));
            db(env)?.prepare("INSERT OR IGNORE INTO memo (uid, creator_id, created_ts, updated_ts, row_status, content, visibility, pinned, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&[
                    uid.clone().into(),
                    js_num(viewer.id),
                    js_num(created_ts),
                    js_num(updated_ts),
                    row_status.into(),
                    content.into(),
                    visibility.into(),
                    js_num(pinned),
                    payload.to_string().into(),
                ])?
                .run()
                .await?;
            if let Some(inserted) = get_memo_by_uid(env, &uid).await? {
                sync_memo_index(env, &inserted).await?;
            }
            imported += 1;
        }
    }
    json_response(json!({ "imported": imported }), 200).map_err(AppError::from)
}

pub(crate) async fn generate_rss(
    env: &Env,
    username: Option<&str>,
) -> std::result::Result<Response, AppError> {
    let rows = if let Some(username) = username {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.row_status = 'NORMAL' AND memo.visibility = 'PUBLIC' AND \"user\".username = ? ORDER BY memo.created_ts DESC LIMIT 50")
            .bind(&[username.into()])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.row_status = 'NORMAL' AND memo.visibility = 'PUBLIC' ORDER BY memo.created_ts DESC LIMIT 50")
            .all()
            .await?
    };
    let memos: Vec<DbMemo> = rows.results()?;
    let items = memos.into_iter().map(|memo| {
        let title = extract_title(&memo.content);
        format!(
            "    <item>\n      <title>{}</title>\n      <link>/memos/{}</link>\n      <guid isPermaLink=\"false\">memos/{}</guid>\n      <pubDate>{}</pubDate>\n      <author>{}</author>\n      <description>{}</description>\n    </item>",
            escape_xml(&title),
            escape_xml(&memo.uid),
            escape_xml(&memo.uid),
            js_sys::Date::new(&JsValue::from_f64((memo.created_ts * 1000) as f64)).to_utc_string().as_string().unwrap_or_default(),
            escape_xml(&memo.creator_username),
            escape_xml(&memo.content),
        )
    }).collect::<Vec<_>>().join("\n");
    let self_path = username
        .map(|name| format!("/api/v1/u/{}/rss.xml", name))
        .unwrap_or_else(|| "/api/v1/explore/rss.xml".to_string());
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n  <channel>\n    <title>Memos Worker</title>\n    <link>/</link>\n    <description>A lightweight memo hub</description>\n    <lastBuildDate>{}</lastBuildDate>\n    <atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\"/>\n{}\n  </channel>\n</rss>",
        js_sys::Date::new_0().to_utc_string().as_string().unwrap_or_default(),
        escape_xml(&self_path),
        items
    );
    let mut response = Response::ok(xml)?;
    response
        .headers_mut()
        .set("Content-Type", "application/rss+xml; charset=utf-8")?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=600")?;
    Ok(response)
}
