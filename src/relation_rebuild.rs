use super::*;

const RELATION_REBUILD_BATCH_SIZE: i64 = 8;
const RELATION_REBUILD_CANDIDATE_LIMIT: i64 = 5000;
const RELATION_REBUILD_AI_CANDIDATE_LIMIT: usize = 45;
const RELATION_REBUILD_MAX_RELATIONS_PER_MEMO: usize = 12;
const RELATION_REBUILD_MIN_LOCAL_SCORE: f64 = 4.0;
const RELATION_REBUILD_MIN_AI_SCORE: f64 = 6.0;

pub(crate) async fn rebuild_relations_batch_route(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let cursor = body
        .get("cursor")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let batch_size = body
        .get("batchSize")
        .and_then(Value::as_i64)
        .unwrap_or(RELATION_REBUILD_BATCH_SIZE)
        .clamp(1, 16);
    let replace_existing = body
        .get("mode")
        .and_then(Value::as_str)
        .map(|mode| mode == "replace")
        .unwrap_or(false);
    let accumulated_created = body
        .get("accumulatedCreated")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let accumulated_updated = body
        .get("accumulatedUpdated")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let accumulated_skipped = body
        .get("accumulatedSkipped")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let progress =
        rebuild_relations_batch(env, viewer, cursor, batch_size, replace_existing).await?;
    if progress.done {
        record_audit(
            env,
            Some(viewer),
            "relations.rebuild",
            "memo_relation",
            json!({
                "processed": progress.processed,
                "total": progress.total,
                "created": accumulated_created + progress.created,
                "updated": accumulated_updated + progress.updated,
                "skipped": accumulated_skipped + progress.skipped,
                "source": progress.source,
                "mode": if replace_existing { "replace" } else { "supplement" }
            }),
        )
        .await;
    }
    json_response(json!({ "progress": progress }), 200).map_err(AppError::from)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelationRebuildProgress {
    pub(crate) total: i64,
    pub(crate) processed: i64,
    pub(crate) batch_processed: i64,
    pub(crate) created: usize,
    pub(crate) updated: usize,
    pub(crate) skipped: usize,
    pub(crate) next_cursor: Option<i64>,
    pub(crate) done: bool,
    pub(crate) source: String,
    pub(crate) warnings: Vec<String>,
}

pub(crate) async fn rebuild_relations_batch(
    env: &Env,
    viewer: &Viewer,
    cursor: i64,
    batch_size: i64,
    replace_existing: bool,
) -> std::result::Result<RelationRebuildProgress, AppError> {
    let total = count_relation_rebuild_memos(env, viewer).await?;
    let batch = list_relation_rebuild_batch(env, viewer, cursor, batch_size).await?;
    let batch_processed = batch.len() as i64;
    let candidates = list_relation_rebuild_candidates(env, viewer).await?;
    let candidate_content: HashMap<String, String> = candidates
        .iter()
        .map(|candidate| (candidate.uid.clone(), candidate.content.clone()))
        .collect();
    let candidate_ids: HashMap<String, i64> = candidates
        .iter()
        .map(|candidate| (candidate.uid.clone(), candidate.id))
        .collect();
    let relation_candidates: Vec<RelationCandidate> = candidates
        .iter()
        .map(relation_candidate_from_memo)
        .collect();
    let settings = resolve_ai_settings(env).await?;
    let use_ai = !settings.api_key.trim().is_empty();
    let mut created = 0;
    let mut updated = 0;
    let mut skipped = 0;
    let mut ai_used = 0;
    let mut warnings = Vec::new();

    for memo in &batch {
        let current = relation_candidate_from_memo(memo);
        let ranked = rank_relation_candidates(
            &current,
            &relation_candidates,
            RELATION_REBUILD_AI_CANDIDATE_LIMIT,
        );
        let top_score = ranked
            .first()
            .map(|candidate| candidate.score)
            .unwrap_or(0.0);
        if ranked.is_empty() || top_score < RELATION_REBUILD_MIN_LOCAL_SCORE {
            if replace_existing {
                replace_memo_relations(env, memo.id, &[]).await?;
            }
            skipped += 1;
            continue;
        }
        let suggestions = if use_ai && top_score >= RELATION_REBUILD_MIN_AI_SCORE {
            match request_ai_relation_suggestions(&settings, memo, &ranked, &candidate_content)
                .await
            {
                Ok(items) if !items.is_empty() => {
                    ai_used += 1;
                    items
                }
                Ok(_) => local_relation_suggestions(&ranked),
                Err(error) => {
                    if warnings.len() < 3 {
                        warnings.push(error.message);
                    }
                    local_relation_suggestions(&ranked)
                }
            }
        } else {
            local_relation_suggestions(&ranked)
        };
        let mut related_ids = suggestion_related_ids(&suggestions, &candidate_ids, &memo.uid);
        let changed =
            apply_memo_relations(env, memo.id, &mut related_ids, replace_existing).await?;
        if changed == 0 {
            skipped += 1;
        } else {
            created += changed;
            updated += 1;
        }
    }

    let next_cursor = batch.last().map(|memo| memo.id);
    let processed =
        count_relation_rebuild_processed(env, viewer, next_cursor.unwrap_or(cursor)).await?;
    let done = batch_processed < batch_size || processed >= total;
    Ok(RelationRebuildProgress {
        total,
        processed,
        batch_processed,
        created,
        updated,
        skipped,
        next_cursor: if done { None } else { next_cursor },
        done,
        source: if use_ai && ai_used > 0 {
            "ai".to_string()
        } else {
            "local".to_string()
        },
        warnings,
    })
}

async fn count_relation_rebuild_memos(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<i64, AppError> {
    db(env)?
        .prepare(
            "SELECT COUNT(*) AS count FROM memo WHERE creator_id = ? AND row_status = 'NORMAL'",
        )
        .bind(&[js_num(viewer.id)])?
        .first(Some("count"))
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to count memos"))
}

async fn count_relation_rebuild_processed(
    env: &Env,
    viewer: &Viewer,
    cursor: i64,
) -> std::result::Result<i64, AppError> {
    db(env)?
        .prepare("SELECT COUNT(*) AS count FROM memo WHERE creator_id = ? AND row_status = 'NORMAL' AND id <= ?")
        .bind(&[js_num(viewer.id), js_num(cursor)])?
        .first(Some("count"))
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to count processed memos"))
}

async fn list_relation_rebuild_batch(
    env: &Env,
    viewer: &Viewer,
    cursor: i64,
    batch_size: i64,
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let rows = db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.creator_id = ? AND memo.row_status = 'NORMAL' AND memo.id > ? ORDER BY memo.id ASC LIMIT ?")
        .bind(&[js_num(viewer.id), js_num(cursor), js_num(batch_size)])?
        .all()
        .await?;
    rows.results().map_err(AppError::from)
}

async fn list_relation_rebuild_candidates(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let rows = db(env)?.prepare("SELECT memo.*, \"user\".username AS creator_username, \"user\".nickname AS creator_nickname FROM memo JOIN \"user\" ON \"user\".id = memo.creator_id WHERE memo.creator_id = ? AND memo.row_status = 'NORMAL' ORDER BY memo.updated_ts DESC, memo.id DESC LIMIT ?")
        .bind(&[js_num(viewer.id), js_num(RELATION_REBUILD_CANDIDATE_LIMIT)])?
        .all()
        .await?;
    rows.results().map_err(AppError::from)
}

fn suggestion_related_ids(
    suggestions: &[Value],
    candidate_ids: &HashMap<String, i64>,
    current_uid: &str,
) -> Vec<i64> {
    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for suggestion in suggestions {
        let uid = suggestion
            .get("memo")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim_start_matches("memos/")
            .trim();
        if uid.is_empty() || uid == current_uid || !seen.insert(uid.to_string()) {
            continue;
        }
        if let Some(id) = candidate_ids.get(uid) {
            ids.push(*id);
        }
    }
    ids
}

async fn replace_memo_relations(
    env: &Env,
    memo_id: i64,
    related_ids: &[i64],
) -> std::result::Result<(), AppError> {
    let database = db(env)?;
    let mut statements = vec![database
        .prepare("DELETE FROM memo_relation WHERE memo_id = ? AND type = 'REFERENCE'")
        .bind(&[js_num(memo_id)])?];
    for related_id in related_ids {
        statements.push(
            database
                .prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'REFERENCE')")
                .bind(&[js_num(memo_id), js_num(*related_id)])?,
        );
    }
    database.batch(statements).await?;
    Ok(())
}

async fn apply_memo_relations(
    env: &Env,
    memo_id: i64,
    related_ids: &mut Vec<i64>,
    replace_existing: bool,
) -> std::result::Result<usize, AppError> {
    if replace_existing {
        related_ids.truncate(RELATION_REBUILD_MAX_RELATIONS_PER_MEMO);
        replace_memo_relations(env, memo_id, related_ids).await?;
        return Ok(related_ids.len());
    }

    let existing = existing_relation_ids(env, memo_id).await?;
    if existing.len() >= RELATION_REBUILD_MAX_RELATIONS_PER_MEMO {
        return Ok(0);
    }
    let remaining = RELATION_REBUILD_MAX_RELATIONS_PER_MEMO - existing.len();
    related_ids.retain(|id| !existing.contains(id));
    related_ids.truncate(remaining);
    if related_ids.is_empty() {
        return Ok(0);
    }
    let database = db(env)?;
    let mut statements = Vec::with_capacity(related_ids.len());
    for related_id in related_ids.iter() {
        statements.push(
            database
                .prepare("INSERT OR IGNORE INTO memo_relation (memo_id, related_memo_id, type) VALUES (?, ?, 'REFERENCE')")
                .bind(&[js_num(memo_id), js_num(*related_id)])?,
        );
    }
    database.batch(statements).await?;
    Ok(related_ids.len())
}

async fn existing_relation_ids(
    env: &Env,
    memo_id: i64,
) -> std::result::Result<BTreeSet<i64>, AppError> {
    let rows = db(env)?
        .prepare(
            "SELECT related_memo_id FROM memo_relation WHERE memo_id = ? AND type = 'REFERENCE'",
        )
        .bind(&[js_num(memo_id)])?
        .all()
        .await?;
    let values: Vec<Value> = rows.results()?;
    Ok(values
        .into_iter()
        .filter_map(|row| row.get("related_memo_id").and_then(Value::as_i64))
        .collect())
}
