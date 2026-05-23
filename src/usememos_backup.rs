use super::*;

pub(crate) async fn backup_to_original_memos(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let result = push_local_memos_to_original(env, viewer, &options).await?;
    record_original_backup_audit(env, viewer, &options, &result).await;
    json_response(json!({ "result": result }), 200).map_err(AppError::from)
}

pub(crate) async fn push_local_memos_to_original(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
) -> std::result::Result<OriginalBackupResult, AppError> {
    let mut rows =
        list_local_memos_for_original_backup(env, viewer, options.include_archived).await?;
    let truncated = rows.len() > MIGRATION_MAX_MEMOS;
    if truncated {
        rows.truncate(MIGRATION_MAX_MEMOS);
    }

    let mut result = OriginalBackupResult {
        memo_count: 0,
        pushed: 0,
        skipped: 0,
        archived_count: 0,
        truncated,
    };

    for memo in rows {
        result.memo_count += 1;
        if memo.row_status == "ARCHIVED" {
            result.archived_count += 1;
        }
        if should_skip_original_backup(&memo, &options.base_url) {
            result.skipped += 1;
            continue;
        }
        let original_name = create_original_backup_memo(options, &memo).await?;
        store_original_backup_marker(env, &memo, options, &original_name).await?;
        result.pushed += 1;
    }

    Ok(result)
}

async fn list_local_memos_for_original_backup(
    env: &Env,
    viewer: &Viewer,
    include_archived: bool,
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let sql = if include_archived {
        "SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.creator_id = ? ORDER BY memo.created_ts, memo.id LIMIT ?"
    } else {
        "SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.creator_id = ? AND memo.row_status = 'NORMAL' ORDER BY memo.created_ts, memo.id LIMIT ?"
    };
    let rows = db(env)?
        .prepare(sql)
        .bind(&[js_num(viewer.id), js_num((MIGRATION_MAX_MEMOS + 1) as i64)])?
        .all()
        .await?;
    rows.results().map_err(AppError::from)
}

pub(crate) fn should_skip_original_backup(memo: &DbMemo, base_url: &str) -> bool {
    if memo.content.trim().is_empty() {
        return true;
    }
    let payload = parse_memo_payload_value(memo);
    let sync = payload
        .get("sync")
        .and_then(|value| value.get("usememos"))
        .and_then(Value::as_object);
    if let Some(sync) = sync {
        let same_base = sync
            .get("baseUrl")
            .and_then(Value::as_str)
            .map(|stored| stored == base_url)
            .unwrap_or(false);
        let original_name = sync
            .get("originalName")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let pushed_at = sync
            .get("pushedAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if same_base && (!original_name.is_empty() || !pushed_at.is_empty()) {
            return true;
        }
    }
    payload
        .get("source")
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        == Some("usememos")
}

async fn create_original_backup_memo(
    options: &MigrationOptions,
    memo: &DbMemo,
) -> std::result::Result<String, AppError> {
    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", options.access_token))?;
    headers.set("Content-Type", "application/json")?;
    let created_at = unix_to_iso_string(memo.created_ts);
    let updated_at = unix_to_iso_string(memo.updated_ts);
    let payload = json!({
        "state": memo.row_status,
        "content": memo.content,
        "visibility": memo.visibility,
        "pinned": memo.pinned != 0,
        "createTime": created_at,
        "updateTime": updated_at,
        "displayTime": created_at
    });
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&payload.to_string())));
    let request = Request::new_with_init(&format!("{}/api/v1/memos", options.base_url), &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(
            400,
            format!(
                "Original Memos API returned HTTP {} while backing up",
                response.status_code()
            ),
        ));
    }
    let data: Value = response.json().await?;
    Ok(data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

async fn store_original_backup_marker(
    env: &Env,
    memo: &DbMemo,
    options: &MigrationOptions,
    original_name: &str,
) -> std::result::Result<(), AppError> {
    let mut payload = parse_memo_payload_value(memo);
    if !payload.is_object() {
        payload = json!({});
    }
    let Some(payload_map) = payload.as_object_mut() else {
        return Ok(());
    };
    let sync = payload_map.entry("sync").or_insert_with(|| json!({}));
    if !sync.is_object() {
        *sync = json!({});
    }
    if let Some(sync_map) = sync.as_object_mut() {
        sync_map.insert(
            "usememos".to_string(),
            json!({
                "baseUrl": options.base_url,
                "originalName": original_name,
                "localUpdatedTs": memo.updated_ts,
                "pushedAt": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default()
            }),
        );
    }
    db(env)?
        .prepare("UPDATE memo SET payload = ? WHERE id = ?")
        .bind(&[payload.to_string().into(), js_num(memo.id)])?
        .run()
        .await?;
    Ok(())
}

fn parse_memo_payload_value(memo: &DbMemo) -> Value {
    serde_json::from_str(&memo.payload).unwrap_or_else(|_| json!({}))
}

fn unix_to_iso_string(ts: i64) -> String {
    js_sys::Date::new(&JsValue::from_f64((ts * 1000) as f64))
        .to_iso_string()
        .as_string()
        .unwrap_or_default()
}

pub(crate) async fn record_original_backup_audit(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    result: &OriginalBackupResult,
) {
    record_audit(
        env,
        Some(viewer),
        "migration.usememos.export",
        "usememos",
        json!({
            "baseUrl": options.base_url,
            "pushed": result.pushed,
            "skipped": result.skipped,
            "memoCount": result.memo_count,
            "archivedCount": result.archived_count,
            "truncated": result.truncated
        }),
    )
    .await;
}
