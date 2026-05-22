fn truncate(value: &str, max_chars: usize) -> String {
    let mut text: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        text.push_str("...");
    }
    text
}

async fn fetch_asset(req: &Request, env: &Env) -> std::result::Result<Response, AppError> {
    let assets = env.assets("ASSETS")?;
    let response = assets.fetch_request(req.clone()?).await?;
    if response.status_code() != 404 {
        return Ok(response);
    }
    let index_req = Request::new_with_init("/index.html", &RequestInit::new())?;
    Ok(assets.fetch_request(index_req).await?)
}

fn db(env: &Env) -> std::result::Result<D1Database, AppError> {
    Ok(env.d1("DB")?)
}

fn server_secret(env: &Env) -> std::result::Result<String, AppError> {
    if let Ok(secret) = env.secret("SERVER_SECRET") {
        return Ok(secret.to_string());
    }
    env.var("SERVER_SECRET").map(|value| value.to_string()).map_err(|_| AppError::new(500, "SERVER_SECRET is not configured"))
}

fn normalize_username(value: Option<&str>) -> std::result::Result<String, AppError> {
    let username = value.unwrap_or("").trim().to_lowercase();
    let valid = username.len() >= 3
        && username.len() <= 32
        && username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        && username.chars().next().map(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_').unwrap_or(false);
    if valid {
        Ok(username)
    } else {
        Err(AppError::new(400, "Username must be 3-32 lowercase letters, numbers, _ or -"))
    }
}

fn assert_password(value: &str) -> std::result::Result<(), AppError> {
    if value.len() >= 8 {
        Ok(())
    } else {
        Err(AppError::new(400, "Password must be at least 8 characters"))
    }
}

fn normalize_visibility(value: &str) -> std::result::Result<String, AppError> {
    let visibility = value.to_uppercase();
    if matches!(visibility.as_str(), "PUBLIC" | "PROTECTED" | "PRIVATE") {
        Ok(visibility)
    } else {
        Err(AppError::new(400, "Invalid visibility"))
    }
}

fn normalize_state(value: &str) -> std::result::Result<String, AppError> {
    let state = value.to_uppercase();
    if matches!(state.as_str(), "NORMAL" | "ARCHIVED") {
        Ok(state)
    } else {
        Err(AppError::new(400, "Invalid row status"))
    }
}

fn hash_password(password: &str) -> String {
    let salt = random_bytes(16);
    let mut derived = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, 100_000, &mut derived);
    format!("pbkdf2_sha256$100000${}${}", base64url(&salt), base64url(&derived))
}

fn verify_password(password: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2_sha256" {
        return false;
    }
    let iterations = parts[1].parse::<u32>().unwrap_or(0);
    if iterations == 0 {
        return false;
    }
    let salt = match URL_SAFE_NO_PAD.decode(parts[2]) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let expected = match URL_SAFE_NO_PAD.decode(parts[3]) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let mut actual = vec![0u8; expected.len()];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iterations, &mut actual);
    constant_time_equal(&actual, &expected)
}

fn sign_jwt(claims: &Claims, secret: &str) -> std::result::Result<String, AppError> {
    let header = base64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = base64url(serde_json::to_string(claims).map_err(|_| AppError::new(500, "JWT encode failed"))?.as_bytes());
    let data = format!("{}.{}", header, payload);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| AppError::new(500, "JWT key failed"))?;
    mac.update(data.as_bytes());
    let signature = mac.finalize().into_bytes();
    Ok(format!("{}.{}", data, base64url(&signature)))
}

fn verify_jwt(token: &str, secret: &str) -> std::result::Result<Claims, AppError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(AppError::new(401, "Invalid token"));
    }
    let data = format!("{}.{}", parts[0], parts[1]);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| AppError::new(500, "JWT key failed"))?;
    mac.update(data.as_bytes());
    let expected = mac.finalize().into_bytes();
    let actual = URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| AppError::new(401, "Invalid token"))?;
    if !constant_time_equal(&actual, &expected) {
        return Err(AppError::new(401, "Invalid token"));
    }
    let payload = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| AppError::new(401, "Invalid token"))?;
    let claims: Claims = serde_json::from_slice(&payload).map_err(|_| AppError::new(401, "Invalid token"))?;
    if claims.iss != "memos-worker" || claims.exp < unix_now() {
        return Err(AppError::new(401, "Invalid token"));
    }
    Ok(claims)
}

fn base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    for byte in &mut bytes {
        *byte = (js_sys::Math::random() * 256.0).floor() as u8;
    }
    bytes
}

fn generate_uid(prefix: &str) -> String {
    format!("{}_{}", prefix, base64url(&random_bytes(12)))
}

fn unix_now() -> i64 {
    js_sys::Date::now() as i64 / 1000
}

fn parse_cookies(header: &str) -> HashMap<String, String> {
    header.split(';')
        .filter_map(|part| {
            let mut chunks = part.trim().splitn(2, '=');
            Some((chunks.next()?.to_string(), chunks.next().unwrap_or("").to_string()))
        })
        .collect()
}

fn cookie(name: &str, value: &str, max_age: i64, http_only: bool) -> String {
    let mut parts = vec![
        format!("{}={}", name, value),
        "Path=/api/v1".to_string(),
        format!("Max-Age={}", max_age),
        "SameSite=Lax".to_string(),
        "Secure".to_string(),
    ];
    if http_only {
        parts.push("HttpOnly".to_string());
    }
    parts.join("; ")
}

fn clear_cookie(name: &str) -> String {
    format!("{}=; Path=/api/v1; Max-Age=0; SameSite=Lax; Secure; HttpOnly", name)
}

fn append_cookie(response: &mut Response, value: &str) {
    let _ = response.headers_mut().append("Set-Cookie", value);
}

fn json_response(body: Value, status: u16) -> Result<Response> {
    let mut response = Response::from_json(&body)?.with_status(status);
    response.headers_mut().set("Access-Control-Allow-Origin", "*")?;
    response.headers_mut().set("Access-Control-Allow-Methods", "GET,POST,PATCH,DELETE,OPTIONS")?;
    response.headers_mut().set("Access-Control-Allow-Headers", "Content-Type,Authorization")?;
    Ok(response)
}

fn empty_response(status: u16) -> Response {
    let mut response = Response::empty().unwrap().with_status(status);
    let _ = response.headers_mut().set("Access-Control-Allow-Origin", "*");
    let _ = response.headers_mut().set("Access-Control-Allow-Methods", "GET,POST,PATCH,DELETE,OPTIONS");
    let _ = response.headers_mut().set("Access-Control-Allow-Headers", "Content-Type,Authorization");
    response
}

fn extract_filter_value(url: &Url, field: &str) -> Option<String> {
    let filter = url.query_pairs().find(|(key, _)| key == "filter")?.1.to_string();
    let needle = format!("{} == \"", field);
    let start = filter.find(&needle)? + needle.len();
    let rest = &filter[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn query_param(url: &Url, field: &str) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key == field)
        .map(|(_, value)| value.to_string())
}

fn extract_content_contains_filter(url: &Url) -> Option<String> {
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

fn escape_like(value: &str) -> String {
    value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn placeholders(count: usize) -> String {
    std::iter::repeat("?").take(count).collect::<Vec<_>>().join(",")
}

fn bind_with_first(first: i64, ids: &[i64]) -> Vec<JsValue> {
    let mut values = vec![js_num(first)];
    values.extend(ids.iter().map(|id| js_num(*id)));
    values
}

fn js_num(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}
