use super::*;

pub(crate) fn original_memo_name(memo: &OriginalMemo) -> String {
    memo.name.as_deref().unwrap_or("").trim().to_string()
}

pub(crate) fn summarize_original_memos(
    memos: &[OriginalMemo],
    truncated: bool,
) -> MigrationSummary {
    let mut summary = MigrationSummary {
        memo_count: 0,
        attachment_count: 0,
        relation_count: 0,
        archived_count: 0,
        truncated,
    };
    for memo in memos {
        summary.memo_count += 1;
        summary.attachment_count += memo.attachments.as_ref().map(Vec::len).unwrap_or(0);
        summary.relation_count += memo.relations.as_ref().map(Vec::len).unwrap_or(0);
        if normalize_original_state(memo.state.as_deref()) == "ARCHIVED" {
            summary.archived_count += 1;
        }
    }
    summary
}

pub(crate) fn build_memo_payload_with_tags(
    content: &str,
    original_tags: Option<&Vec<String>>,
) -> Value {
    let mut payload = build_memo_payload(content);
    if let Some(tags) = original_tags {
        if let Some(existing) = payload.get_mut("tags").and_then(Value::as_array_mut) {
            for tag in tags
                .iter()
                .map(|tag| tag.trim())
                .filter(|tag| !tag.is_empty())
            {
                if !existing.iter().any(|value| value.as_str() == Some(tag)) {
                    existing.push(json!(tag));
                }
            }
        }
    }
    payload
}

pub(crate) fn parse_original_timestamp(value: Option<&Value>, fallback: i64) -> i64 {
    match value {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(fallback),
        Some(Value::String(text)) if !text.trim().is_empty() => {
            let parsed = js_sys::Date::parse(text);
            if parsed.is_finite() {
                (parsed / 1000.0).floor() as i64
            } else {
                fallback
            }
        }
        _ => fallback,
    }
}

pub(crate) fn normalize_original_state(value: Option<&str>) -> String {
    let state = value
        .unwrap_or("NORMAL")
        .to_ascii_uppercase()
        .replace("STATE_", "");
    match state.as_str() {
        "" | "UNSPECIFIED" => "NORMAL".to_string(),
        "DELETED" => "ARCHIVED".to_string(),
        "ARCHIVED" => "ARCHIVED".to_string(),
        _ => "NORMAL".to_string(),
    }
}

pub(crate) fn normalize_original_visibility(value: Option<&str>) -> String {
    let visibility = value
        .unwrap_or("PRIVATE")
        .to_ascii_uppercase()
        .replace("VISIBILITY_", "");
    match visibility.as_str() {
        "PUBLIC" | "PROTECTED" | "PRIVATE" => visibility,
        _ => "PRIVATE".to_string(),
    }
}
