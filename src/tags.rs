use super::*;

pub(crate) async fn list_tags(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let memos = get_recent_memos(env, viewer, 500).await?;
    let mut counts: HashMap<String, i64> = HashMap::new();
    for memo in memos {
        if let Ok(payload) = serde_json::from_str::<Value>(&memo.payload) {
            if let Some(tags) = payload.get("tags").and_then(Value::as_array) {
                for tag in tags.iter().filter_map(Value::as_str) {
                    *counts.entry(tag.to_string()).or_default() += 1;
                }
            }
        }
    }
    let tags: Vec<Value> = counts
        .into_iter()
        .map(|(name, count)| json!({ "name": name, "count": count }))
        .collect();
    json_response(json!({ "tags": tags }), 200).map_err(AppError::from)
}

pub(crate) async fn rename_tag(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let from = normalize_tag_name(body.get("from").and_then(Value::as_str).unwrap_or(""));
    let to = normalize_tag_name(body.get("to").and_then(Value::as_str).unwrap_or(""));
    if from.is_empty() || to.is_empty() {
        return Err(AppError::new(400, "from and to are required"));
    }
    let rows = if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.content LIKE ?")
            .bind(&[format!("%#{}%", from).into()])?
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.content LIKE ? AND memo.creator_id = ?")
            .bind(&[format!("%#{}%", from).into(), js_num(viewer.id)])?
            .all()
            .await?
    };
    let memos: Vec<DbMemo> = rows.results()?;
    let mut updated = 0;
    for memo in memos {
        let next_content = replace_tag_in_content(&memo.content, &from, &to);
        if next_content == memo.content {
            continue;
        }
        db(env)?
            .prepare("UPDATE memo SET content = ?, payload = ?, updated_ts = ? WHERE id = ?")
            .bind(&[
                next_content.clone().into(),
                build_memo_payload(&next_content).to_string().into(),
                js_num(unix_now()),
                js_num(memo.id),
            ])?
            .run()
            .await?;
        updated += 1;
    }
    json_response(json!({ "updated": updated }), 200).map_err(AppError::from)
}

pub(crate) async fn timeline(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let rows = if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT date(created_ts, 'unixepoch') AS day, COUNT(*) AS count FROM memo WHERE row_status = 'NORMAL' GROUP BY day ORDER BY day DESC LIMIT 120")
            .all().await?
    } else {
        db(env)?.prepare("SELECT date(created_ts, 'unixepoch') AS day, COUNT(*) AS count FROM memo WHERE row_status = 'NORMAL' AND (visibility != 'PRIVATE' OR creator_id = ?) GROUP BY day ORDER BY day DESC LIMIT 120")
            .bind(&[js_num(viewer.id)])?
            .all().await?
    };
    let days: Vec<Value> = rows.results()?;
    json_response(json!({ "days": days }), 200).map_err(AppError::from)
}
