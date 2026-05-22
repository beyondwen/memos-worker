use super::*;

pub(crate) fn json_response(body: Value, status: u16) -> Result<Response> {
    let mut response = Response::from_json(&body)?.with_status(status);
    response
        .headers_mut()
        .set("Access-Control-Allow-Origin", "*")?;
    response.headers_mut().set(
        "Access-Control-Allow-Methods",
        "GET,POST,PATCH,DELETE,OPTIONS",
    )?;
    response
        .headers_mut()
        .set("Access-Control-Allow-Headers", "Content-Type,Authorization")?;
    Ok(response)
}

pub(crate) fn empty_response(status: u16) -> Response {
    let mut response = Response::empty().unwrap().with_status(status);
    let _ = response
        .headers_mut()
        .set("Access-Control-Allow-Origin", "*");
    let _ = response.headers_mut().set(
        "Access-Control-Allow-Methods",
        "GET,POST,PATCH,DELETE,OPTIONS",
    );
    let _ = response
        .headers_mut()
        .set("Access-Control-Allow-Headers", "Content-Type,Authorization");
    response
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
