use super::*;
use serde::Deserialize;

const RELATION_REBUILD_BATCH_SIZE: i64 = 16;
const RELATION_REBUILD_SCHEDULER_BATCH_SIZE: i64 = 32;
const RELATION_REBUILD_SNAPSHOT_PAGE_SIZE: i64 = 200;
const RELATION_REBUILD_AI_CANDIDATE_LIMIT: usize = 45;
const RELATION_REBUILD_RECALL_LIMIT: i64 = 240;
const RELATION_REBUILD_TOKEN_LIMIT_PER_MEMO: usize = 32;
const RELATION_REBUILD_TOKEN_INSERT_BATCH_SIZE: usize = 200;
const RELATION_REBUILD_TOKEN_INDEX_PAGE_SIZE: i64 = 200;
const RELATION_REBUILD_MAX_AI_CALLS_PER_STEP: usize = 4;
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
    let action = body.get("action").and_then(Value::as_str).unwrap_or("step");
    if action == "start" {
        let replace_existing = relation_rebuild_replace_existing(&body);
        let task = start_relation_rebuild_task(env, viewer, replace_existing).await?;
        return json_response(json!({ "progress": task.to_progress() }), 200)
            .map_err(AppError::from);
    }
    if action == "latest" {
        let task = latest_relation_rebuild_task(env, viewer.id).await?;
        return json_response(
            json!({ "progress": task.map(|task| task.to_progress()) }),
            200,
        )
        .map_err(AppError::from);
    }
    let task_id = body
        .get("taskId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if task_id.is_empty() {
        return Err(AppError::new(400, "Task ID is required"));
    }
    if action == "status" {
        let task = get_relation_rebuild_task(env, task_id, Some(viewer.id)).await?;
        return json_response(json!({ "progress": task.to_progress() }), 200)
            .map_err(AppError::from);
    }
    let batch_size = body
        .get("batchSize")
        .and_then(Value::as_i64)
        .unwrap_or(RELATION_REBUILD_BATCH_SIZE)
        .clamp(1, 32);
    let task = process_relation_rebuild_task_safely(
        env,
        task_id,
        Some(viewer.id),
        batch_size,
        Some(viewer),
    )
    .await?;
    let progress = task.to_progress();
    json_response(json!({ "progress": progress }), 200).map_err(AppError::from)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelationRebuildProgress {
    pub(crate) task_id: String,
    pub(crate) status: String,
    pub(crate) mode: String,
    pub(crate) total: i64,
    pub(crate) processed: i64,
    pub(crate) batch_processed: i64,
    pub(crate) created: usize,
    pub(crate) updated: usize,
    pub(crate) skipped: usize,
    pub(crate) next_cursor: Option<i64>,
    pub(crate) done: bool,
    pub(crate) source: String,
    pub(crate) error: Option<String>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct DbRelationRebuildTask {
    id: String,
    user_id: i64,
    status: String,
    mode: String,
    cursor: i64,
    total: i64,
    processed: i64,
    created: i64,
    updated: i64,
    skipped: i64,
    source: String,
    error: String,
    warnings: String,
}

impl DbRelationRebuildTask {
    fn to_progress(&self) -> RelationRebuildProgress {
        RelationRebuildProgress {
            task_id: self.id.clone(),
            status: self.status.clone(),
            mode: self.mode.clone(),
            total: self.total,
            processed: self.processed,
            batch_processed: 0,
            created: self.created.max(0) as usize,
            updated: self.updated.max(0) as usize,
            skipped: self.skipped.max(0) as usize,
            next_cursor: if matches!(
                self.status.as_str(),
                "SNAPSHOTTING" | "INDEXING" | "RUNNING"
            ) {
                Some(self.cursor)
            } else {
                None
            },
            done: !matches!(
                self.status.as_str(),
                "SNAPSHOTTING" | "INDEXING" | "RUNNING"
            ),
            source: self.source.clone(),
            error: if self.error.trim().is_empty() {
                None
            } else {
                Some(self.error.clone())
            },
            warnings: parse_relation_rebuild_warnings(&self.warnings),
        }
    }
}

pub(crate) async fn process_pending_relation_rebuild_tasks(
    env: &Env,
) -> std::result::Result<(), AppError> {
    ensure_relation_rebuild_tables(env).await?;
    let rows = db(env)?
        .prepare("SELECT * FROM relation_rebuild_task WHERE status IN ('SNAPSHOTTING', 'INDEXING', 'RUNNING') ORDER BY updated_ts ASC, created_ts ASC LIMIT 2")
        .all()
        .await?;
    let tasks: Vec<DbRelationRebuildTask> = rows.results()?;
    for task in tasks {
        let _ = process_relation_rebuild_task_safely(
            env,
            &task.id,
            None,
            RELATION_REBUILD_SCHEDULER_BATCH_SIZE,
            None,
        )
        .await;
    }
    Ok(())
}

fn relation_rebuild_replace_existing(body: &Value) -> bool {
    body.get("mode")
        .and_then(Value::as_str)
        .map(|mode| mode == "replace")
        .unwrap_or(false)
}

async fn start_relation_rebuild_task(
    env: &Env,
    viewer: &Viewer,
    replace_existing: bool,
) -> std::result::Result<DbRelationRebuildTask, AppError> {
    ensure_relation_rebuild_tables(env).await?;
    let task_id = generate_uid("rr")?;
    let mode = if replace_existing {
        "replace"
    } else {
        "supplement"
    };
    let now = unix_now();
    let database = db(env)?;
    database
        .prepare("UPDATE relation_rebuild_task SET status = 'CANCELED', updated_ts = ? WHERE user_id = ? AND status IN ('SNAPSHOTTING', 'INDEXING', 'RUNNING')")
        .bind(&[js_num(now), js_num(viewer.id)])?
        .run()
        .await?;
    database
        .prepare("DELETE FROM relation_rebuild_candidate_token WHERE task_id IN (SELECT id FROM relation_rebuild_task WHERE user_id = ? AND status NOT IN ('SNAPSHOTTING', 'INDEXING', 'RUNNING'))")
        .bind(&[js_num(viewer.id)])?
        .run()
        .await?;
    database
        .prepare("DELETE FROM relation_rebuild_candidate WHERE task_id IN (SELECT id FROM relation_rebuild_task WHERE user_id = ? AND status NOT IN ('SNAPSHOTTING', 'INDEXING', 'RUNNING'))")
        .bind(&[js_num(viewer.id)])?
        .run()
        .await?;
    database
        .prepare("INSERT INTO relation_rebuild_task (id, user_id, created_ts, updated_ts, status, mode, cursor, total, processed, created, updated, skipped, source, failed_attempts, error, warnings) VALUES (?, ?, ?, ?, 'SNAPSHOTTING', ?, 0, 0, 0, 0, 0, 0, 'local', 0, '', '[]')")
        .bind(&[
            task_id.clone().into(),
            js_num(viewer.id),
            js_num(now),
            js_num(now),
            mode.into(),
        ])?
        .run()
        .await?;
    let total = count_relation_rebuild_source_memos(env, viewer.id).await?;
    database
        .prepare("UPDATE relation_rebuild_task SET total = ?, updated_ts = ? WHERE id = ?")
        .bind(&[js_num(total), js_num(unix_now()), task_id.clone().into()])?
        .run()
        .await?;
    get_relation_rebuild_task(env, &task_id, Some(viewer.id)).await
}

async fn process_relation_rebuild_task(
    env: &Env,
    task_id: &str,
    owner_id: Option<i64>,
    batch_size: i64,
    actor: Option<&Viewer>,
) -> std::result::Result<DbRelationRebuildTask, AppError> {
    ensure_relation_rebuild_tables(env).await?;
    let task = get_relation_rebuild_task(env, task_id, owner_id).await?;
    if task.status == "SNAPSHOTTING" {
        return process_relation_rebuild_snapshot_batch(env, &task).await;
    }
    if task.status == "INDEXING" {
        return process_relation_rebuild_index_batch(env, &task).await;
    }
    if task.status != "RUNNING" {
        return Ok(task);
    }
    let replace_existing = task.mode == "replace";
    let batch = list_relation_rebuild_task_batch(env, &task.id, task.cursor, batch_size).await?;
    let batch_processed = batch.len() as i64;
    let settings = resolve_ai_settings(env).await?;
    let use_ai = !settings.api_key.trim().is_empty();
    let mut created = 0usize;
    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut ai_used = false;
    let mut ai_calls = 0usize;
    let mut warnings = parse_relation_rebuild_warnings(&task.warnings);

    for memo in &batch {
        let candidates = list_relation_rebuild_recall_candidates(
            env,
            &task.id,
            memo,
            RELATION_REBUILD_RECALL_LIMIT,
        )
        .await?;
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
        let suggestions = if use_ai
            && top_score >= RELATION_REBUILD_MIN_AI_SCORE
            && ai_calls < RELATION_REBUILD_MAX_AI_CALLS_PER_STEP
        {
            match request_ai_relation_suggestions(&settings, memo, &ranked, &candidate_content)
                .await
            {
                Ok(items) if !items.is_empty() => {
                    ai_used = true;
                    ai_calls += 1;
                    items
                }
                Ok(_) => {
                    ai_calls += 1;
                    local_relation_suggestions(&ranked)
                }
                Err(error) => {
                    ai_calls += 1;
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

    let next_cursor = batch.last().map(|memo| memo.id).unwrap_or(task.cursor);
    let processed = task.processed + batch_processed;
    let done = batch_processed < batch_size || processed >= task.total;
    let status = if done { "DONE" } else { "RUNNING" };
    let source = if task.source == "ai" || ai_used {
        "ai"
    } else {
        "local"
    };
    db(env)?
        .prepare("UPDATE relation_rebuild_task SET updated_ts = ?, status = ?, cursor = ?, processed = ?, created = created + ?, updated = updated + ?, skipped = skipped + ?, source = ?, failed_attempts = 0, error = '', warnings = ? WHERE id = ?")
        .bind(&[
            js_num(unix_now()),
            status.into(),
            js_num(next_cursor),
            js_num(processed),
            js_num(created as i64),
            js_num(updated as i64),
            js_num(skipped as i64),
            source.into(),
            json!(warnings).to_string().into(),
            task.id.clone().into(),
        ])?
        .run()
        .await?;
    let updated_task = get_relation_rebuild_task(env, &task.id, owner_id).await?;
    if done {
        record_relation_rebuild_audit(env, actor, &updated_task).await;
        let _ = cleanup_relation_rebuild_candidates(env, &task.id).await;
    }
    Ok(updated_task)
}

async fn process_relation_rebuild_task_safely(
    env: &Env,
    task_id: &str,
    owner_id: Option<i64>,
    batch_size: i64,
    actor: Option<&Viewer>,
) -> std::result::Result<DbRelationRebuildTask, AppError> {
    match process_relation_rebuild_task(env, task_id, owner_id, batch_size, actor).await {
        Ok(task) => Ok(task),
        Err(error) => match get_relation_rebuild_task(env, task_id, owner_id).await {
            Ok(task)
                if matches!(
                    task.status.as_str(),
                    "SNAPSHOTTING" | "INDEXING" | "RUNNING"
                ) =>
            {
                fail_relation_rebuild_task(env, &task, &error.message).await
            }
            _ => Err(error),
        },
    }
}

async fn fail_relation_rebuild_task(
    env: &Env,
    task: &DbRelationRebuildTask,
    message: &str,
) -> std::result::Result<DbRelationRebuildTask, AppError> {
    let error = truncate(message, 480);
    let mut warnings = parse_relation_rebuild_warnings(&task.warnings);
    if warnings.last().map(String::as_str) != Some(error.as_str()) {
        warnings.push(error.clone());
        if warnings.len() > 5 {
            warnings = warnings[warnings.len() - 5..].to_vec();
        }
    }
    db(env)?
        .prepare("UPDATE relation_rebuild_task SET status = 'FAILED', updated_ts = ?, failed_attempts = failed_attempts + 1, error = ?, warnings = ? WHERE id = ?")
        .bind(&[
            js_num(unix_now()),
            error.into(),
            json!(warnings).to_string().into(),
            task.id.clone().into(),
        ])?
        .run()
        .await?;
    let _ = cleanup_relation_rebuild_candidates(env, &task.id).await;
    get_relation_rebuild_task(env, &task.id, None).await
}

async fn ensure_relation_rebuild_tables(env: &Env) -> std::result::Result<(), AppError> {
    let database = db(env)?;
    database
        .prepare("CREATE TABLE IF NOT EXISTS relation_rebuild_task (id TEXT PRIMARY KEY, user_id INTEGER NOT NULL, created_ts INTEGER NOT NULL, updated_ts INTEGER NOT NULL, status TEXT NOT NULL DEFAULT 'RUNNING', mode TEXT NOT NULL DEFAULT 'supplement', cursor INTEGER NOT NULL DEFAULT 0, total INTEGER NOT NULL DEFAULT 0, processed INTEGER NOT NULL DEFAULT 0, created INTEGER NOT NULL DEFAULT 0, updated INTEGER NOT NULL DEFAULT 0, skipped INTEGER NOT NULL DEFAULT 0, source TEXT NOT NULL DEFAULT 'local', failed_attempts INTEGER NOT NULL DEFAULT 0, error TEXT NOT NULL DEFAULT '', warnings TEXT NOT NULL DEFAULT '[]')")
        .run()
        .await?;
    database
        .prepare("CREATE INDEX IF NOT EXISTS idx_relation_rebuild_task_status ON relation_rebuild_task(status, updated_ts)")
        .run()
        .await?;
    database
        .prepare("CREATE INDEX IF NOT EXISTS idx_relation_rebuild_task_user ON relation_rebuild_task(user_id, created_ts)")
        .run()
        .await?;
    database
        .prepare("CREATE TABLE IF NOT EXISTS relation_rebuild_candidate (task_id TEXT NOT NULL, memo_id INTEGER NOT NULL, user_id INTEGER NOT NULL, uid TEXT NOT NULL, created_ts INTEGER NOT NULL, updated_ts INTEGER NOT NULL, content TEXT NOT NULL DEFAULT '', visibility TEXT NOT NULL DEFAULT 'PRIVATE', pinned INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '{}', PRIMARY KEY (task_id, memo_id))")
        .run()
        .await?;
    database
        .prepare("CREATE INDEX IF NOT EXISTS idx_relation_rebuild_candidate_user ON relation_rebuild_candidate(task_id, user_id)")
        .run()
        .await?;
    database
        .prepare("CREATE TABLE IF NOT EXISTS relation_rebuild_candidate_token (task_id TEXT NOT NULL, token TEXT NOT NULL, memo_id INTEGER NOT NULL, PRIMARY KEY (task_id, token, memo_id))")
        .run()
        .await?;
    database
        .prepare("CREATE INDEX IF NOT EXISTS idx_relation_rebuild_candidate_token_memo ON relation_rebuild_candidate_token(task_id, memo_id)")
        .run()
        .await?;
    Ok(())
}

async fn get_relation_rebuild_task(
    env: &Env,
    task_id: &str,
    owner_id: Option<i64>,
) -> std::result::Result<DbRelationRebuildTask, AppError> {
    let row = db(env)?
        .prepare("SELECT * FROM relation_rebuild_task WHERE id = ?")
        .bind(&[task_id.into()])?
        .first::<DbRelationRebuildTask>(None)
        .await?
        .ok_or_else(|| AppError::new(404, "Relation rebuild task not found"))?;
    if owner_id.is_some_and(|id| id != row.user_id) {
        return Err(AppError::new(403, "Forbidden relation rebuild task"));
    }
    Ok(row)
}

async fn latest_relation_rebuild_task(
    env: &Env,
    user_id: i64,
) -> std::result::Result<Option<DbRelationRebuildTask>, AppError> {
    ensure_relation_rebuild_tables(env).await?;
    db(env)?
        .prepare("SELECT * FROM relation_rebuild_task WHERE user_id = ? ORDER BY created_ts DESC LIMIT 1")
        .bind(&[js_num(user_id)])?
        .first::<DbRelationRebuildTask>(None)
        .await
        .map_err(AppError::from)
}

async fn count_relation_rebuild_source_memos(
    env: &Env,
    user_id: i64,
) -> std::result::Result<i64, AppError> {
    db(env)?
        .prepare(
            "SELECT COUNT(*) AS count FROM memo WHERE creator_id = ? AND row_status = 'NORMAL'",
        )
        .bind(&[js_num(user_id)])?
        .first(Some("count"))
        .await?
        .ok_or_else(|| AppError::new(500, "Failed to count relation rebuild source memos"))
}

async fn list_relation_rebuild_task_batch(
    env: &Env,
    task_id: &str,
    cursor: i64,
    batch_size: i64,
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let rows = db(env)?.prepare("SELECT memo_id AS id, uid, user_id AS creator_id, '' AS creator_username, '' AS creator_nickname, created_ts, updated_ts, 'NORMAL' AS row_status, content, visibility, pinned, payload FROM relation_rebuild_candidate WHERE task_id = ? AND memo_id > ? ORDER BY memo_id ASC LIMIT ?")
        .bind(&[task_id.into(), js_num(cursor), js_num(batch_size)])?
        .all()
        .await?;
    rows.results().map_err(AppError::from)
}

async fn list_relation_rebuild_snapshot_batch(
    env: &Env,
    user_id: i64,
    cursor: i64,
    limit: i64,
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let rows = db(env)?.prepare("SELECT memo.*, '' AS creator_username, '' AS creator_nickname FROM memo WHERE memo.creator_id = ? AND memo.row_status = 'NORMAL' AND memo.id > ? ORDER BY memo.id ASC LIMIT ?")
        .bind(&[js_num(user_id), js_num(cursor), js_num(limit)])?
        .all()
        .await?;
    rows.results().map_err(AppError::from)
}

async fn list_relation_rebuild_task_candidates_page(
    env: &Env,
    task_id: &str,
    cursor: i64,
    limit: i64,
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let rows = db(env)?.prepare("SELECT memo_id AS id, uid, user_id AS creator_id, '' AS creator_username, '' AS creator_nickname, created_ts, updated_ts, 'NORMAL' AS row_status, content, visibility, pinned, payload FROM relation_rebuild_candidate WHERE task_id = ? AND memo_id > ? ORDER BY memo_id ASC LIMIT ?")
        .bind(&[task_id.into(), js_num(cursor), js_num(limit)])?
        .all()
        .await?;
    rows.results().map_err(AppError::from)
}

async fn process_relation_rebuild_snapshot_batch(
    env: &Env,
    task: &DbRelationRebuildTask,
) -> std::result::Result<DbRelationRebuildTask, AppError> {
    let memos = list_relation_rebuild_snapshot_batch(
        env,
        task.user_id,
        task.cursor,
        RELATION_REBUILD_SNAPSHOT_PAGE_SIZE,
    )
    .await?;
    if memos.is_empty() {
        db(env)?
        .prepare("UPDATE relation_rebuild_task SET status = 'INDEXING', cursor = 0, processed = 0, failed_attempts = 0, error = '', updated_ts = ? WHERE id = ?")
            .bind(&[js_num(unix_now()), task.id.clone().into()])?
            .run()
            .await?;
        return get_relation_rebuild_task(env, &task.id, None).await;
    }

    let next_cursor = memos.last().map(|memo| memo.id).unwrap_or(task.cursor);
    let inserted = memos.len() as i64;
    let database = db(env)?;
    let mut statements = Vec::with_capacity(memos.len());
    for memo in memos {
        statements.push(
            database
                .prepare("INSERT OR IGNORE INTO relation_rebuild_candidate (task_id, memo_id, user_id, uid, created_ts, updated_ts, content, visibility, pinned, payload) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(&[
                    task.id.clone().into(),
                    js_num(memo.id),
                    js_num(memo.creator_id),
                    memo.uid.into(),
                    js_num(memo.created_ts),
                    js_num(memo.updated_ts),
                    memo.content.into(),
                    memo.visibility.into(),
                    js_num(memo.pinned),
                    memo.payload.into(),
                ])?,
        );
    }
    database.batch(statements).await?;
    database
        .prepare("UPDATE relation_rebuild_task SET cursor = ?, processed = processed + ?, failed_attempts = 0, error = '', updated_ts = ? WHERE id = ? AND status = 'SNAPSHOTTING'")
        .bind(&[
            js_num(next_cursor),
            js_num(inserted),
            js_num(unix_now()),
            task.id.clone().into(),
        ])?
        .run()
        .await?;
    get_relation_rebuild_task(env, &task.id, None).await
}

async fn process_relation_rebuild_index_batch(
    env: &Env,
    task: &DbRelationRebuildTask,
) -> std::result::Result<DbRelationRebuildTask, AppError> {
    let candidates = list_relation_rebuild_task_candidates_page(
        env,
        &task.id,
        task.cursor,
        RELATION_REBUILD_TOKEN_INDEX_PAGE_SIZE,
    )
    .await?;
    if candidates.is_empty() {
        db(env)?
            .prepare("UPDATE relation_rebuild_task SET status = 'RUNNING', cursor = 0, processed = 0, failed_attempts = 0, error = '', updated_ts = ? WHERE id = ?")
            .bind(&[js_num(unix_now()), task.id.clone().into()])?
            .run()
            .await?;
        return get_relation_rebuild_task(env, &task.id, None).await;
    }

    let next_cursor = candidates
        .last()
        .map(|candidate| candidate.id)
        .unwrap_or(task.cursor);
    let indexed = candidates.len() as i64;
    let database = db(env)?;
    let mut statements = Vec::new();
    for candidate in candidates {
        let relation_candidate = relation_candidate_from_memo(&candidate);
        for token in relation_candidate_search_terms(
            &relation_candidate,
            RELATION_REBUILD_TOKEN_LIMIT_PER_MEMO,
        ) {
            statements.push(
                database
                    .prepare("INSERT OR IGNORE INTO relation_rebuild_candidate_token (task_id, token, memo_id) VALUES (?, ?, ?)")
                    .bind(&[task.id.clone().into(), token.into(), js_num(candidate.id)])?,
            );
            if statements.len() >= RELATION_REBUILD_TOKEN_INSERT_BATCH_SIZE {
                database.batch(std::mem::take(&mut statements)).await?;
            }
        }
    }
    if !statements.is_empty() {
        database.batch(statements).await?;
    }
    database
        .prepare("UPDATE relation_rebuild_task SET cursor = ?, processed = processed + ?, failed_attempts = 0, error = '', updated_ts = ? WHERE id = ? AND status = 'INDEXING'")
        .bind(&[
            js_num(next_cursor),
            js_num(indexed),
            js_num(unix_now()),
            task.id.clone().into(),
        ])?
        .run()
        .await?;
    get_relation_rebuild_task(env, &task.id, None).await
}

async fn list_relation_rebuild_recall_candidates(
    env: &Env,
    task_id: &str,
    memo: &DbMemo,
    limit: i64,
) -> std::result::Result<Vec<DbMemo>, AppError> {
    let current = relation_candidate_from_memo(memo);
    let tokens = relation_candidate_search_terms(&current, RELATION_REBUILD_TOKEN_LIMIT_PER_MEMO);
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = placeholders(tokens.len());
    let mut values: Vec<JsValue> = Vec::with_capacity(tokens.len() + 3);
    values.push(task_id.into());
    values.extend(tokens.into_iter().map(JsValue::from));
    values.push(js_num(memo.id));
    values.push(js_num(limit));
    let rows = db(env)?
        .prepare(format!(
            "SELECT c.memo_id AS id, c.uid, c.user_id AS creator_id, '' AS creator_username, '' AS creator_nickname, c.created_ts, c.updated_ts, 'NORMAL' AS row_status, c.content, c.visibility, c.pinned, c.payload FROM relation_rebuild_candidate_token t JOIN relation_rebuild_candidate c ON c.task_id = t.task_id AND c.memo_id = t.memo_id WHERE t.task_id = ? AND t.token IN ({}) AND c.memo_id != ? GROUP BY c.memo_id ORDER BY COUNT(*) DESC, c.updated_ts DESC, c.memo_id DESC LIMIT ?",
            placeholders
        ))
        .bind(&values)?
        .all()
        .await?;
    rows.results().map_err(AppError::from)
}

async fn cleanup_relation_rebuild_candidates(
    env: &Env,
    task_id: &str,
) -> std::result::Result<(), AppError> {
    let database = db(env)?;
    database
        .prepare("DELETE FROM relation_rebuild_candidate_token WHERE task_id = ?")
        .bind(&[task_id.into()])?
        .run()
        .await?;
    database
        .prepare("DELETE FROM relation_rebuild_candidate WHERE task_id = ?")
        .bind(&[task_id.into()])?
        .run()
        .await?;
    Ok(())
}

fn parse_relation_rebuild_warnings(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

async fn record_relation_rebuild_audit(
    env: &Env,
    actor: Option<&Viewer>,
    task: &DbRelationRebuildTask,
) {
    record_audit(
        env,
        actor,
        "relations.rebuild",
        "memo_relation",
        json!({
            "processed": task.processed,
            "total": task.total,
            "created": task.created,
            "updated": task.updated,
            "skipped": task.skipped,
            "source": task.source,
            "mode": task.mode
        }),
    )
    .await;
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
