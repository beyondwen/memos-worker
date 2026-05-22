use super::*;

pub(crate) async fn get_ai_settings(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let settings = resolve_ai_settings(env).await?;
    json_response(json!({ "settings": public_ai_settings(&settings) }), 200).map_err(AppError::from)
}

pub(crate) async fn update_ai_settings(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    let previous = read_stored_ai_settings(env).await?;
    let next = merge_ai_settings(&previous, &body)?;
    db(env)?.prepare("INSERT INTO system_setting (name, value, description) VALUES ('ai.settings', ?, 'AI model settings') ON CONFLICT(name) DO UPDATE SET value = excluded.value")
        .bind(&[serde_json::to_string(&next).map_err(|error| AppError::new(500, error.to_string()))?.into()])?
        .run()
        .await?;
    json_response(json!({ "settings": public_ai_settings(&next) }), 200).map_err(AppError::from)
}

pub(crate) async fn test_ai_settings(
    req: &mut Request,
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    let body: Value = req.json().await.unwrap_or_else(|_| json!({}));
    let base = resolve_ai_settings(env).await?;
    let settings = merge_ai_settings(&base, &body)?;
    if settings.api_key.trim().is_empty() {
        return Err(AppError::new(400, "AI API Key is required"));
    }
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", settings.api_key))?;
    headers.set("Content-Type", "application/json")?;
    let payload = json!({
        "model": settings.model,
        "temperature": 0,
        "messages": [
            { "role": "system", "content": "Return ok." },
            { "role": "user", "content": "ping" }
        ],
        "max_tokens": 8
    });
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&payload.to_string())));
    let request = Request::new_with_init(
        &format!(
            "{}/chat/completions",
            settings.base_url.trim_end_matches('/')
        ),
        &init,
    )?;
    let response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(
            502,
            format!("AI API returned HTTP {}", response.status_code()),
        ));
    }
    json_response(json!({ "ok": true }), 200).map_err(AppError::from)
}

pub(crate) async fn resolve_ai_settings(env: &Env) -> std::result::Result<AiSettings, AppError> {
    let stored = read_stored_ai_settings(env).await?;
    Ok(AiSettings {
        base_url: normalize_http_url(
            if stored.base_url.is_empty() {
                env.var("AI_BASE_URL")
                    .map(|value| value.to_string())
                    .unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
            } else {
                stored.base_url
            },
            "Invalid AI Base URL",
        )?,
        model: if stored.model.is_empty() {
            env.var("AI_MODEL")
                .map(|value| value.to_string())
                .unwrap_or_else(|_| "gpt-4o-mini".to_string())
        } else {
            stored.model
        },
        api_key: if stored.api_key.is_empty() {
            env.secret("AI_API_KEY")
                .map(|value| value.to_string())
                .unwrap_or_default()
        } else {
            stored.api_key
        },
    })
}

pub(crate) async fn read_stored_ai_settings(
    env: &Env,
) -> std::result::Result<AiSettings, AppError> {
    let value: Option<String> = db(env)?
        .prepare("SELECT value FROM system_setting WHERE name = 'ai.settings'")
        .first(Some("value"))
        .await?;
    let stored = value.and_then(|text| serde_json::from_str::<AiSettings>(&text).ok());
    Ok(stored.unwrap_or(AiSettings {
        base_url: "https://api.openai.com/v1".to_string(),
        model: "gpt-4o-mini".to_string(),
        api_key: String::new(),
    }))
}

pub(crate) fn merge_ai_settings(
    previous: &AiSettings,
    update: &Value,
) -> std::result::Result<AiSettings, AppError> {
    let base_url = update
        .get("baseUrl")
        .and_then(Value::as_str)
        .unwrap_or(&previous.base_url);
    let model = update
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(&previous.model)
        .trim();
    let api_key = update
        .get("apiKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&previous.api_key);
    Ok(AiSettings {
        base_url: normalize_http_url(base_url, "Invalid AI Base URL")?,
        model: if model.is_empty() {
            "gpt-4o-mini".to_string()
        } else {
            model.to_string()
        },
        api_key: api_key.to_string(),
    })
}

pub(crate) fn public_ai_settings(settings: &AiSettings) -> Value {
    json!({
        "baseUrl": settings.base_url,
        "model": settings.model,
        "configured": !settings.api_key.trim().is_empty()
    })
}
