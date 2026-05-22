use super::*;

pub(crate) async fn bulk_memos(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let action = body
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_uppercase();
    let mut seen = BTreeSet::new();
    let uids: Vec<String> = body
        .get("memoUids")
        .and_then(Value::as_array)
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|uid| !uid.is_empty())
        .filter(|uid| seen.insert(uid.to_string()))
        .map(ToString::to_string)
        .take(200)
        .collect();
    if uids.is_empty() {
        return Err(AppError::new(400, "memoUids is required"));
    }
    let memos = get_memos_by_uids(env, viewer, &uids).await?;
    let ids: Vec<i64> = memos.iter().map(|memo| memo.id).collect();
    let result =
        json!({ "updated": 0, "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) });
    if ids.is_empty() {
        return json_response(result, 200).map_err(AppError::from);
    }
    let placeholders = placeholders(ids.len());
    let now = unix_now();
    match action.as_str() {
        "ARCHIVE" => {
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET row_status = 'ARCHIVED', updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&bind_with_first(now, &ids))?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "ARCHIVE",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                Some("ARCHIVED"),
                None,
            )
            .await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "RESTORE" => {
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET row_status = 'NORMAL', updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&bind_with_first(now, &ids))?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "RESTORE",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                Some("NORMAL"),
                None,
            )
            .await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "DELETE" => {
            purge_ids(env, &ids).await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "DELETE",
                0,
                ids.len(),
                uids.len().saturating_sub(ids.len()),
                now,
                None,
                None,
            )
            .await;
            json_response(json!({ "updated": 0, "deleted": ids.len(), "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        "VISIBILITY" => {
            let visibility =
                normalize_visibility(body.get("visibility").and_then(Value::as_str).unwrap_or(""))?;
            let mut values = vec![visibility.clone().into(), js_num(now)];
            values.extend(ids.iter().map(|id| js_num(*id)));
            db(env)?
                .prepare(format!(
                    "UPDATE memo SET visibility = ?, updated_ts = ? WHERE id IN ({})",
                    placeholders
                ))
                .bind(&values)?
                .run()
                .await?;
            emit_bulk_memo_events(
                env,
                &memos,
                "VISIBILITY",
                ids.len(),
                0,
                uids.len().saturating_sub(ids.len()),
                now,
                None,
                Some(&visibility),
            )
            .await;
            json_response(json!({ "updated": ids.len(), "deleted": 0, "skipped": uids.len().saturating_sub(ids.len()) }), 200).map_err(AppError::from)
        }
        _ => Err(AppError::new(400, "Invalid bulk action")),
    }
}
