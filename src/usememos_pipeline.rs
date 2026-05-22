use super::*;

pub(crate) async fn import_original_memos(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    mut events: Option<&mut Vec<String>>,
) -> std::result::Result<MigrationProgress, AppError> {
    let (memos, truncated) = fetch_original_memos(options).await?;
    let summary = summarize_original_memos(&memos, truncated);
    let mut progress = MigrationProgress {
        phase: "importing".to_string(),
        processed: 0,
        imported: 0,
        skipped: 0,
        memo_count: summary.memo_count,
        attachment_count: summary.attachment_count,
        relation_count: summary.relation_count,
        archived_count: summary.archived_count,
        truncated,
        state: None,
    };
    if let Some(buf) = events.as_deref_mut() {
        buf.push(sse_event("progress", &progress)?);
    }
    for memo in memos {
        if import_single_original_memo(env, viewer, &memo).await? {
            progress.imported += 1;
        } else {
            progress.skipped += 1;
        }
        progress.processed += 1;
        if let Some(buf) = events.as_deref_mut() {
            buf.push(sse_event("progress", &progress)?);
        }
    }
    progress.phase = "done".to_string();
    if let Some(buf) = events.as_deref_mut() {
        buf.push(sse_event("progress", &progress)?);
    }
    Ok(progress)
}

pub(crate) async fn import_original_memos_streaming<F>(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    mut on_progress: F,
) -> std::result::Result<MigrationProgress, AppError>
where
    F: FnMut(&str, &MigrationProgress) -> std::result::Result<(), AppError>,
{
    let states = if options.include_archived {
        vec!["NORMAL", "ARCHIVED"]
    } else {
        vec!["NORMAL"]
    };
    let mut progress = MigrationProgress {
        phase: "fetching".to_string(),
        processed: 0,
        imported: 0,
        skipped: 0,
        memo_count: 0,
        attachment_count: 0,
        relation_count: 0,
        archived_count: 0,
        truncated: false,
        state: None,
    };
    on_progress("progress", &progress)?;
    let mut imported_original_names = BTreeSet::new();

    for state in states {
        let mut page_token = String::new();
        loop {
            let previous_page_token = page_token.clone();
            progress.phase = "fetching".to_string();
            progress.state = Some(state.to_string());
            on_progress("progress", &progress)?;

            let mut url = Url::parse(&format!("{}/api/v1/memos", options.base_url))
                .map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
            url.query_pairs_mut()
                .append_pair("pageSize", &MIGRATION_PAGE_SIZE.to_string())
                .append_pair("state", state);
            if !page_token.is_empty() {
                url.query_pairs_mut().append_pair("pageToken", &page_token);
            }

            let (memos, next_page_token) = fetch_original_memos_page(options, url.as_str()).await?;
            let existing_names = existing_imported_original_names(env, viewer.id, &memos).await?;
            progress.phase = "importing".to_string();
            for memo in memos {
                if progress.memo_count >= MIGRATION_MAX_MEMOS {
                    progress.truncated = true;
                    break;
                }
                progress.memo_count += 1;
                progress.attachment_count += memo.attachments.as_ref().map(Vec::len).unwrap_or(0);
                progress.relation_count += memo.relations.as_ref().map(Vec::len).unwrap_or(0);
                if normalize_original_state(memo.state.as_deref()) == "ARCHIVED" {
                    progress.archived_count += 1;
                }
                let original_name = original_memo_name(&memo);
                let already_imported = !original_name.is_empty()
                    && (existing_names.contains(&original_name)
                        || imported_original_names.contains(&original_name));
                if import_single_original_memo_inner(env, viewer, &memo, already_imported).await? {
                    if !original_name.is_empty() {
                        imported_original_names.insert(original_name);
                    }
                    progress.imported += 1;
                } else {
                    progress.skipped += 1;
                }
                progress.processed += 1;
                on_progress("progress", &progress)?;
            }

            if progress.truncated || next_page_token.is_empty() {
                break;
            }
            if next_page_token == previous_page_token {
                return Err(AppError::new(
                    400,
                    "Original Memos API returned a repeated page token",
                ));
            }
            page_token = next_page_token;
        }
        if progress.truncated {
            break;
        }
    }

    progress.phase = "done".to_string();
    on_progress("progress", &progress)?;
    Ok(progress)
}
