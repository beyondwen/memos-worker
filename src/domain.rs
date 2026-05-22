use super::*;

pub(crate) fn build_memo_payload(content: &str) -> Value {
    let tags: Vec<String> = content
        .split_whitespace()
        .filter_map(|word| word.strip_prefix('#'))
        .map(|tag| {
            tag.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '/')
                .to_string()
        })
        .filter(|tag| !tag.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    json!({
        "tags": tags,
        "property": {
            "hasTaskList": content.contains("- [") || content.contains("* ["),
            "hasLink": content.contains("http://") || content.contains("https://"),
            "hasCode": content.contains("```") || content.contains('`'),
            "hasIncompleteTasks": content.contains("[ ]")
        }
    })
}

pub(crate) fn normalize_tag_name(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('#')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(64)
        .collect()
}

pub(crate) fn replace_tag_in_content(content: &str, from: &str, to: &str) -> String {
    content
        .split_inclusive(char::is_whitespace)
        .map(|token| {
            let trimmed = token.trim_end();
            let suffix = &token[trimmed.len()..];
            if trimmed == format!("#{}", from) {
                format!("#{}{}", to, suffix)
            } else {
                token.to_string()
            }
        })
        .collect()
}

pub(crate) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(crate) fn extract_title(content: &str) -> String {
    let first = content
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('#')
        .trim();
    let cleaned = first.replace(['*', '_', '`', '~'], "");
    if cleaned.is_empty() {
        "Memo".to_string()
    } else {
        let mut title: String = cleaned.chars().take(120).collect();
        if cleaned.chars().count() > 120 {
            title.push_str("...");
        }
        title
    }
}

pub(crate) trait JsValueFallback {
    fn if_undefined(self, fallback: &str) -> JsValue;
}

impl JsValueFallback for JsValue {
    fn if_undefined(self, fallback: &str) -> JsValue {
        if self.is_null() || self.is_undefined() {
            fallback.into()
        } else {
            self
        }
    }
}

pub(crate) fn json_bind(value: Option<&Value>) -> JsValue {
    match value {
        Some(Value::String(text)) => text.clone().into(),
        Some(Value::Number(number)) => number
            .as_i64()
            .map(js_num)
            .unwrap_or_else(|| JsValue::from_f64(number.as_f64().unwrap_or_default())),
        Some(Value::Bool(value)) => js_num(if *value { 1 } else { 0 }),
        Some(Value::Null) | None => JsValue::NULL,
        Some(other) => other.to_string().into(),
    }
}

pub(crate) fn normalize_http_url(
    value: impl AsRef<str>,
    message: &str,
) -> std::result::Result<String, AppError> {
    let raw = value.as_ref().trim();
    if raw.is_empty() {
        return Err(AppError::new(400, message));
    }
    let mut url = Url::parse(raw).map_err(|_| AppError::new(400, message))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(AppError::new(400, message));
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub(crate) async fn record_audit(
    env: &Env,
    viewer: Option<&Viewer>,
    action: &str,
    target: &str,
    detail: Value,
) {
    if ensure_audit_log_table(env).await.is_err() {
        return;
    }
    if let Ok(database) = db(env) {
        let stmt = database.prepare("INSERT INTO audit_log (created_ts, actor_id, action, target, detail) VALUES (?, ?, ?, ?, ?)");
        if let Ok(bound) = stmt.bind(&[
            js_num(unix_now()),
            viewer.map(|user| js_num(user.id)).unwrap_or(JsValue::NULL),
            action.into(),
            target.into(),
            detail.to_string().into(),
        ]) {
            let _ = bound.run().await;
        }
    }
}

pub(crate) async fn ensure_audit_log_table(env: &Env) -> std::result::Result<(), AppError> {
    db(env)?.prepare("CREATE TABLE IF NOT EXISTS audit_log (id INTEGER PRIMARY KEY AUTOINCREMENT, created_ts INTEGER NOT NULL, actor_id INTEGER, action TEXT NOT NULL, target TEXT NOT NULL DEFAULT '', detail TEXT NOT NULL DEFAULT '{}')")
        .run()
        .await?;
    db(env)?
        .prepare("CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_ts)")
        .run()
        .await?;
    db(env)?
        .prepare("CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action, created_ts)")
        .run()
        .await?;
    Ok(())
}

pub(crate) fn audit_action_label(action: &str) -> &str {
    match action {
        "memo.delete" => "删除备忘录",
        "memo.purge" => "彻底删除备忘录",
        "attachment.delete" => "删除附件",
        "backup.create" => "创建备份",
        "backup.restore" => "恢复备份",
        "backup.restore_failed" => "恢复备份失败",
        "memo_index.rebuild" => "重建 Memo 索引",
        "migration.usememos.start" => "开始迁移原版 Memos",
        "migration.usememos.import" => "迁移原版 Memos",
        "migration.usememos.error" => "迁移原版 Memos 失败",
        "tag.rename" => "重命名标签",
        _ => action,
    }
}
