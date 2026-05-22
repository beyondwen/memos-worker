use super::*;

pub(crate) fn json_response(body: Value, status: u16) -> Result<Response> {
    Ok(Response::from_json(&body)?.with_status(status))
}

pub(crate) fn empty_response(status: u16) -> Response {
    Response::empty().unwrap().with_status(status)
}

pub(crate) fn apply_cors(response: &mut Response, req: &Request, env: &Env) -> Result<()> {
    let headers = response.headers_mut();
    headers.delete("Access-Control-Allow-Origin")?;
    headers.set(
        "Access-Control-Allow-Methods",
        "GET,POST,PATCH,DELETE,OPTIONS",
    )?;
    headers.set(
        "Access-Control-Allow-Headers",
        "Content-Type,Authorization,Last-Event-ID",
    )?;
    headers.set("Access-Control-Allow-Credentials", "true")?;
    headers.set("Access-Control-Expose-Headers", "Content-Disposition")?;
    headers.set("Vary", "Origin")?;

    let origin = req.headers().get("Origin").ok().flatten();
    let self_origin = req.url().ok().map(|url| url.origin().ascii_serialization());
    let configured =
        env_text(env, "CORS_ALLOWED_ORIGINS").or_else(|| env_text(env, "ALLOWED_ORIGINS"));

    if let Some(allowed) = allowed_cors_origin(
        origin.as_deref(),
        self_origin.as_deref(),
        configured.as_deref(),
    ) {
        headers.set("Access-Control-Allow-Origin", &allowed)?;
    }

    Ok(())
}

pub(crate) fn apply_security_headers(response: &mut Response) -> Result<()> {
    let headers = response.headers_mut();
    headers.set("X-Content-Type-Options", "nosniff")?;
    headers.set("Referrer-Policy", "strict-origin-when-cross-origin")?;
    headers.set(
        "Permissions-Policy",
        "camera=(), microphone=(), geolocation=(), payment=()",
    )?;
    headers.set("X-Frame-Options", "DENY")?;
    headers.set("Content-Security-Policy", security_content_policy())?;
    Ok(())
}

pub(crate) fn security_content_policy() -> &'static str {
    "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' data:; connect-src 'self' https:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'; manifest-src 'self'"
}

pub(crate) fn env_text(env: &Env, name: &str) -> Option<String> {
    env.var(name)
        .map(|value| value.to_string())
        .or_else(|_| env.secret(name).map(|value| value.to_string()))
        .ok()
}

pub(crate) fn allowed_cors_origin(
    origin: Option<&str>,
    self_origin: Option<&str>,
    configured: Option<&str>,
) -> Option<String> {
    let origin = origin?.trim();
    if origin.is_empty() {
        return None;
    }
    if self_origin
        .map(|value| value.trim() == origin)
        .unwrap_or(false)
    {
        return Some(origin.to_string());
    }
    let configured = configured.unwrap_or("");
    if configured
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .any(|value| value == "*" || value == origin)
    {
        Some(origin.to_string())
    } else {
        None
    }
}

pub(crate) fn extract_filter_value(url: &Url, field: &str) -> Option<String> {
    let filter = url
        .query_pairs()
        .find(|(key, _)| key == "filter")?
        .1
        .to_string();
    let needle = format!("{} == \"", field);
    let start = filter.find(&needle)? + needle.len();
    let rest = &filter[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub(crate) fn query_param(url: &Url, field: &str) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key == field)
        .map(|(_, value)| value.to_string())
}

pub(crate) fn extract_content_contains_filter(url: &Url) -> Option<String> {
    let filter = query_param(url, "filter")?;
    let needle = "content.contains(\"";
    let start = filter.find(needle)? + needle.len();
    let rest = &filter[start..];
    let mut value = String::new();
    let mut escaped = false;
    for ch in rest.chars() {
        if escaped {
            value.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(value);
        } else {
            value.push(ch);
        }
    }
    None
}

pub(crate) fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub(crate) fn placeholders(count: usize) -> String {
    std::iter::repeat("?")
        .take(count)
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn bind_with_first(first: i64, ids: &[i64]) -> Vec<JsValue> {
    let mut values = vec![js_num(first)];
    values.extend(ids.iter().map(|id| js_num(*id)));
    values
}
