use super::*;

pub(crate) async fn list_tags(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    ensure_memo_index_tables(env).await?;
    let rows = if viewer.role == "ADMIN" {
        db(env)?.prepare("SELECT memo_tag.tag AS name, COUNT(*) AS count FROM memo_tag JOIN memo ON memo.id = memo_tag.memo_id WHERE memo.row_status = 'NORMAL' GROUP BY memo_tag.tag ORDER BY count DESC, memo_tag.tag ASC LIMIT 500")
            .all()
            .await?
    } else {
        db(env)?.prepare("SELECT memo_tag.tag AS name, COUNT(*) AS count FROM memo_tag JOIN memo ON memo.id = memo_tag.memo_id WHERE memo.row_status = 'NORMAL' AND (memo.visibility != 'PRIVATE' OR memo.creator_id = ?) GROUP BY memo_tag.tag ORDER BY count DESC, memo_tag.tag ASC LIMIT 500")
            .bind(&[js_num(viewer.id)])?
            .all()
            .await?
    };
    let tags: Vec<Value> = rows.results()?;
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
        let updated_memo = get_memo_by_uid(env, &memo.uid)
            .await?
            .ok_or_else(|| AppError::new(500, "Memo disappeared"))?;
        sync_memo_index(env, &updated_memo).await?;
        updated += 1;
    }
    json_response(json!({ "updated": updated }), 200).map_err(AppError::from)
}

pub(crate) async fn timeline(
    env: &Env,
    url: &Url,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let year = query_param(url, "year").and_then(|value| value.parse::<i32>().ok());
    let month = query_param(url, "month").and_then(|value| value.parse::<u32>().ok());
    let rows = if let (Some(year), Some(month)) = (year, month) {
        let (start_ts, end_ts) = calendar_month_bounds(year, month)?;
        if viewer.role == "ADMIN" {
            db(env)?.prepare("SELECT date(created_ts, 'unixepoch') AS day, COUNT(*) AS count FROM memo WHERE row_status = 'NORMAL' AND created_ts >= ? AND created_ts < ? GROUP BY day ORDER BY day ASC")
                .bind(&[js_num(start_ts), js_num(end_ts)])?
                .all().await?
        } else {
            db(env)?.prepare("SELECT date(created_ts, 'unixepoch') AS day, COUNT(*) AS count FROM memo WHERE row_status = 'NORMAL' AND created_ts >= ? AND created_ts < ? AND (visibility != 'PRIVATE' OR creator_id = ?) GROUP BY day ORDER BY day ASC")
                .bind(&[js_num(start_ts), js_num(end_ts), js_num(viewer.id)])?
                .all().await?
        }
    } else if viewer.role == "ADMIN" {
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

pub(crate) fn calendar_month_bounds(
    year: i32,
    month: u32,
) -> std::result::Result<(i64, i64), AppError> {
    if !(1970..=2100).contains(&year) || !(1..=12).contains(&month) {
        return Err(AppError::new(400, "Invalid calendar month"));
    }
    let start = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .ok_or_else(|| AppError::new(400, "Invalid calendar month"))?
        .and_utc()
        .timestamp();
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .ok_or_else(|| AppError::new(400, "Invalid calendar month"))?
        .and_utc()
        .timestamp();
    Ok((start, end))
}
