use super::*;

pub(crate) fn parse_cookies(header: &str) -> HashMap<String, String> {
    header
        .split(';')
        .filter_map(|part| {
            let mut chunks = part.trim().splitn(2, '=');
            Some((
                chunks.next()?.to_string(),
                chunks.next().unwrap_or("").to_string(),
            ))
        })
        .collect()
}

pub(crate) fn cookie(name: &str, value: &str, max_age: i64, http_only: bool) -> String {
    cookie_at_path(name, value, max_age, http_only, "/api/v1")
}

pub(crate) fn cookie_at_path(
    name: &str,
    value: &str,
    max_age: i64,
    http_only: bool,
    path: &str,
) -> String {
    let mut parts = vec![
        format!("{}={}", name, value),
        format!("Path={}", path),
        format!("Max-Age={}", max_age),
        "SameSite=Lax".to_string(),
        "Secure".to_string(),
    ];
    if http_only {
        parts.push("HttpOnly".to_string());
    }
    parts.join("; ")
}

pub(crate) fn clear_cookie(name: &str) -> String {
    clear_cookie_at_path(name, "/api/v1", true)
}

pub(crate) fn clear_cookie_at_path(name: &str, path: &str, http_only: bool) -> String {
    let mut parts = vec![
        format!("{}=", name),
        format!("Path={}", path),
        "Max-Age=0".to_string(),
        "SameSite=Lax".to_string(),
        "Secure".to_string(),
    ];
    if http_only {
        parts.push("HttpOnly".to_string());
    }
    parts.join("; ")
}

pub(crate) fn append_cookie(response: &mut Response, value: &str) {
    let _ = response.headers_mut().append("Set-Cookie", value);
}
