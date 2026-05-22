use super::*;

pub(crate) fn truncate(value: &str, max_chars: usize) -> String {
    let mut text: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        text.push_str("...");
    }
    text
}

pub(crate) fn db(env: &Env) -> std::result::Result<D1Database, AppError> {
    Ok(env.d1("DB")?)
}

pub(crate) fn unix_now() -> i64 {
    js_sys::Date::now() as i64 / 1000
}

pub(crate) fn js_num(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}

pub(crate) fn normalize_username(value: Option<&str>) -> std::result::Result<String, AppError> {
    let username = value.unwrap_or("").trim().to_lowercase();
    let valid = username.len() >= 3
        && username.len() <= 32
        && username
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        && username
            .chars()
            .next()
            .map(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            .unwrap_or(false);
    if valid {
        Ok(username)
    } else {
        Err(AppError::new(
            400,
            "Username must be 3-32 lowercase letters, numbers, _ or -",
        ))
    }
}

pub(crate) fn assert_password(value: &str) -> std::result::Result<(), AppError> {
    if value.len() >= 8 {
        Ok(())
    } else {
        Err(AppError::new(400, "Password must be at least 8 characters"))
    }
}

pub(crate) fn normalize_visibility(value: &str) -> std::result::Result<String, AppError> {
    let visibility = value.to_uppercase();
    if matches!(visibility.as_str(), "PUBLIC" | "PROTECTED" | "PRIVATE") {
        Ok(visibility)
    } else {
        Err(AppError::new(400, "Invalid visibility"))
    }
}

pub(crate) fn normalize_state(value: &str) -> std::result::Result<String, AppError> {
    let state = value.to_uppercase();
    if matches!(state.as_str(), "NORMAL" | "ARCHIVED") {
        Ok(state)
    } else {
        Err(AppError::new(400, "Invalid row status"))
    }
}
