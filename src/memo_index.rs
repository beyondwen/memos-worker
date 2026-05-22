use super::*;

pub(crate) async fn ensure_memo_index_tables(env: &Env) -> std::result::Result<(), AppError> {
    db(env)?.prepare("CREATE TABLE IF NOT EXISTS memo_search (memo_id INTEGER PRIMARY KEY, content TEXT NOT NULL DEFAULT '', updated_ts INTEGER NOT NULL DEFAULT 0, FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE CASCADE)")
        .run()
        .await?;
    db(env)?.prepare("CREATE TABLE IF NOT EXISTS memo_tag (memo_id INTEGER NOT NULL, tag TEXT NOT NULL, PRIMARY KEY (memo_id, tag), FOREIGN KEY (memo_id) REFERENCES memo(id) ON DELETE CASCADE)")
        .run()
        .await?;
    db(env)?
        .prepare("CREATE INDEX IF NOT EXISTS idx_memo_tag_tag ON memo_tag(tag, memo_id)")
        .run()
        .await?;
    db(env)?
        .prepare("CREATE INDEX IF NOT EXISTS idx_memo_search_updated_ts ON memo_search(updated_ts)")
        .run()
        .await?;
    Ok(())
}

pub(crate) async fn sync_memo_index(env: &Env, memo: &DbMemo) -> std::result::Result<(), AppError> {
    sync_memo_index_fields(env, memo.id, &memo.content, &memo.payload, memo.updated_ts).await
}

pub(crate) async fn sync_memo_index_fields(
    env: &Env,
    memo_id: i64,
    content: &str,
    payload: &str,
    updated_ts: i64,
) -> std::result::Result<(), AppError> {
    ensure_memo_index_tables(env).await?;
    db(env)?
        .prepare(
            "INSERT OR REPLACE INTO memo_search (memo_id, content, updated_ts) VALUES (?, ?, ?)",
        )
        .bind(&[js_num(memo_id), content.into(), js_num(updated_ts)])?
        .run()
        .await?;
    db(env)?
        .prepare("DELETE FROM memo_tag WHERE memo_id = ?")
        .bind(&[js_num(memo_id)])?
        .run()
        .await?;
    for tag in memo_tags_from_payload(payload) {
        db(env)?
            .prepare("INSERT OR IGNORE INTO memo_tag (memo_id, tag) VALUES (?, ?)")
            .bind(&[js_num(memo_id), tag.into()])?
            .run()
            .await?;
    }
    Ok(())
}

pub(crate) async fn delete_memo_indexes(
    env: &Env,
    ids: &[i64],
) -> std::result::Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    ensure_memo_index_tables(env).await?;
    let placeholders = placeholders(ids.len());
    let values: Vec<JsValue> = ids.iter().map(|id| js_num(*id)).collect();
    db(env)?
        .prepare(format!(
            "DELETE FROM memo_search WHERE memo_id IN ({})",
            placeholders
        ))
        .bind(&values)?
        .run()
        .await?;
    db(env)?
        .prepare(format!(
            "DELETE FROM memo_tag WHERE memo_id IN ({})",
            placeholders
        ))
        .bind(&values)?
        .run()
        .await?;
    Ok(())
}

pub(crate) async fn rebuild_memo_index_route(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let rebuilt = rebuild_memo_indexes(env).await?;
    let health = memo_index_health(env).await?;
    record_audit(
        env,
        Some(viewer),
        "memo_index.rebuild",
        "memo_index",
        json!({ "rebuilt": rebuilt, "health": health.to_json() }),
    )
    .await;
    json_response(
        json!({ "rebuilt": rebuilt, "memoIndex": health.to_json() }),
        200,
    )
    .map_err(AppError::from)
}

pub(crate) async fn rebuild_memo_indexes(env: &Env) -> std::result::Result<usize, AppError> {
    ensure_memo_index_tables(env).await?;
    db(env)?.prepare("DELETE FROM memo_search").run().await?;
    db(env)?.prepare("DELETE FROM memo_tag").run().await?;
    let rows = db(env)?
        .prepare("SELECT * FROM memo ORDER BY id ASC")
        .all()
        .await?;
    let memos: Vec<DbMemo> = rows.results()?;
    let count = memos.len();
    for memo in memos {
        sync_memo_index(env, &memo).await?;
    }
    Ok(count)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoIndexHealth {
    pub(crate) memo_count: i64,
    pub(crate) search_count: i64,
    pub(crate) missing_search_count: i64,
    pub(crate) orphan_search_count: i64,
    pub(crate) tag_count: i64,
    pub(crate) orphan_tag_count: i64,
    pub(crate) healthy: bool,
}

impl MemoIndexHealth {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "memoCount": self.memo_count,
            "searchCount": self.search_count,
            "missingSearchCount": self.missing_search_count,
            "orphanSearchCount": self.orphan_search_count,
            "tagCount": self.tag_count,
            "orphanTagCount": self.orphan_tag_count,
            "healthy": self.healthy
        })
    }
}

pub(crate) async fn memo_index_health(env: &Env) -> std::result::Result<MemoIndexHealth, AppError> {
    ensure_memo_index_tables(env).await?;
    let memo_count = query_count(env, "SELECT COUNT(*) AS count FROM memo").await?;
    let search_count = query_count(env, "SELECT COUNT(*) AS count FROM memo_search").await?;
    let missing_search_count = query_count(env, "SELECT COUNT(*) AS count FROM memo LEFT JOIN memo_search ON memo_search.memo_id = memo.id WHERE memo_search.memo_id IS NULL").await?;
    let orphan_search_count = query_count(env, "SELECT COUNT(*) AS count FROM memo_search LEFT JOIN memo ON memo.id = memo_search.memo_id WHERE memo.id IS NULL").await?;
    let tag_count = query_count(env, "SELECT COUNT(*) AS count FROM memo_tag").await?;
    let orphan_tag_count = query_count(env, "SELECT COUNT(*) AS count FROM memo_tag LEFT JOIN memo ON memo.id = memo_tag.memo_id WHERE memo.id IS NULL").await?;
    Ok(MemoIndexHealth {
        memo_count,
        search_count,
        missing_search_count,
        orphan_search_count,
        tag_count,
        orphan_tag_count,
        healthy: memo_count == search_count
            && missing_search_count == 0
            && orphan_search_count == 0
            && orphan_tag_count == 0,
    })
}

async fn query_count(env: &Env, sql: &str) -> std::result::Result<i64, AppError> {
    let count: Option<i64> = db(env)?.prepare(sql).first(Some("count")).await?;
    Ok(count.unwrap_or(0))
}

pub(crate) fn memo_tags_from_payload(payload: &str) -> Vec<String> {
    let parsed = serde_json::from_str::<Value>(payload).unwrap_or_else(|_| json!({}));
    let mut seen = BTreeSet::new();
    parsed
        .get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .filter(|tag| seen.insert((*tag).to_string()))
        .map(ToString::to_string)
        .collect()
}
