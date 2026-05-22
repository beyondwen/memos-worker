use super::*;

pub(crate) async fn migration_preview(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let (memos, truncated) = fetch_original_memos(&options).await?;
    let preview = summarize_original_memos(&memos, truncated);
    let _ = env;
    json_response(json!({ "preview": preview }), 200).map_err(AppError::from)
}

pub(crate) async fn migration_import(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let progress = import_original_memos(env, viewer, &options, None).await?;
    record_migration_audit(env, viewer, &options, &progress).await;
    json_response(json!({ "result": progress }), 200).map_err(AppError::from)
}

pub(crate) async fn migration_import_stream(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let options = read_migration_request(req).await?;
    let env = env.clone();
    let viewer = viewer.clone();
    let (sender, receiver) = mpsc::unbounded::<Vec<u8>>();
    wasm_bindgen_futures::spawn_local(async move {
        let mut sender = sender;
        record_migration_start_audit(&env, &viewer, &options).await;
        let result = import_original_memos_streaming(&env, &viewer, &options, |event, progress| {
            send_sse_chunk(&mut sender, event, progress)
        })
        .await;
        match result {
            Ok(progress) => {
                record_migration_audit(&env, &viewer, &options, &progress).await;
                let _ = send_sse_chunk(&mut sender, "done", &progress);
            }
            Err(error) => {
                record_migration_error_audit(&env, &viewer, &options, &error.message).await;
                let _ = send_sse_chunk(&mut sender, "error", &json!({ "error": error.message }));
            }
        }
    });

    let stream = receiver.map(|chunk| Ok::<Vec<u8>, worker::Error>(chunk));
    let mut response = Response::from_stream(stream)?;
    response
        .headers_mut()
        .set("Content-Type", "text/event-stream; charset=utf-8")?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    response.headers_mut().set("X-Accel-Buffering", "no")?;
    Ok(response)
}

pub(crate) async fn read_migration_request(
    req: &mut Request,
) -> std::result::Result<MigrationOptions, AppError> {
    let body: MigrationRequest = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let raw = body.base_url.unwrap_or_default();
    let mut base_url =
        Url::parse(raw.trim()).map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
    if base_url.scheme() != "http" && base_url.scheme() != "https" {
        return Err(AppError::new(400, "Only http and https URLs are supported"));
    }
    base_url.set_query(None);
    base_url.set_fragment(None);
    let mut base = base_url.to_string();
    while base.ends_with('/') {
        base.pop();
    }
    let access_token = body.access_token.unwrap_or_default().trim().to_string();
    if access_token.is_empty() {
        return Err(AppError::new(400, "Access token is required"));
    }
    Ok(MigrationOptions {
        base_url: base,
        access_token,
        include_archived: body.include_archived.unwrap_or(false),
    })
}

pub(crate) async fn fetch_original_memos(
    options: &MigrationOptions,
) -> std::result::Result<(Vec<OriginalMemo>, bool), AppError> {
    let mut all = Vec::new();
    let mut truncated = false;
    let states = if options.include_archived {
        vec!["NORMAL", "ARCHIVED"]
    } else {
        vec!["NORMAL"]
    };
    for state in states {
        let mut page_token = String::new();
        loop {
            let mut url = Url::parse(&format!("{}/api/v1/memos", options.base_url))
                .map_err(|_| AppError::new(400, "Invalid Memos URL"))?;
            url.query_pairs_mut()
                .append_pair("pageSize", &MIGRATION_PAGE_SIZE.to_string())
                .append_pair("state", state);
            if !page_token.is_empty() {
                url.query_pairs_mut().append_pair("pageToken", &page_token);
            }
            let (memos, next_page_token) = fetch_original_memos_page(options, url.as_str()).await?;
            for memo in memos {
                if all.len() >= MIGRATION_MAX_MEMOS {
                    truncated = true;
                    break;
                }
                all.push(memo);
            }
            if truncated || next_page_token.is_empty() {
                break;
            }
            page_token = next_page_token;
        }
        if truncated {
            break;
        }
    }
    Ok((all, truncated))
}

pub(crate) async fn fetch_original_memos_page(
    options: &MigrationOptions,
    url: &str,
) -> std::result::Result<(Vec<OriginalMemo>, String), AppError> {
    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", options.access_token))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(url, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(
            400,
            format!(
                "Original Memos API returned HTTP {}",
                response.status_code()
            ),
        ));
    }
    let data: Value = response.json().await?;
    let memos = data
        .get("memos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| serde_json::from_value::<OriginalMemo>(value).ok())
        .collect();
    let next = data
        .get("nextPageToken")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok((memos, next))
}
