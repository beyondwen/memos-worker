use super::*;

pub(crate) async fn create_backup_artifact(
    env: &Env,
) -> std::result::Result<BackupArtifact, AppError> {
    let key = backup_key();
    let plaintext = serde_json::to_string_pretty(&build_backup_payload(env).await?)
        .map_err(|error| AppError::new(500, error.to_string()))?;
    let body = backup_storage_body(env, &plaintext).await?;
    let size = body.text.as_bytes().len();
    env.bucket("MEMOS_BUCKET")?
        .put(key.clone(), body.text)
        .http_metadata(HttpMetadata {
            content_type: Some("application/json".to_string()),
            ..Default::default()
        })
        .execute()
        .await?;
    Ok(BackupArtifact {
        key,
        size,
        encrypted: body.encrypted,
        key_id: body.key_id,
    })
}

pub(crate) fn backup_artifact_payload(artifact: &BackupArtifact) -> Value {
    json!({
        "backup": {
            "key": artifact.key,
            "size": artifact.size,
            "encrypted": artifact.encrypted,
            "keyId": artifact.key_id
        }
    })
}

pub(crate) async fn read_backup_payload(
    req: &mut Request,
    env: &Env,
) -> std::result::Result<Value, AppError> {
    let body: Value = req
        .json()
        .await
        .map_err(|_| AppError::new(400, "Invalid JSON"))?;
    if let Some(payload) = body.get("payload") {
        return Ok(payload.clone());
    }
    let key = body.get("key").and_then(Value::as_str).unwrap_or("");
    if !key.starts_with("backups/") {
        return Err(AppError::new(400, "Invalid backup key"));
    }
    let object = env
        .bucket("MEMOS_BUCKET")?
        .get(key.to_string())
        .execute()
        .await?
        .ok_or_else(|| AppError::new(404, "Backup not found"))?;
    let text = object
        .body()
        .ok_or_else(|| AppError::new(404, "Backup not found"))?
        .text()
        .await?;
    let text = backup_plaintext(env, &text).await?;
    serde_json::from_str(&text).map_err(|_| AppError::new(400, "Invalid backup payload"))
}

pub(crate) async fn build_backup_payload(env: &Env) -> std::result::Result<Value, AppError> {
    let users = db(env)?.prepare("SELECT id, created_ts, updated_ts, row_status, username, role, email, nickname, avatar_url, description FROM \"user\" ORDER BY id").all().await?.results::<Value>()?;
    let memos = db(env)?
        .prepare("SELECT * FROM memo ORDER BY created_ts, id")
        .all()
        .await?
        .results::<Value>()?;
    let attachments = db(env)?.prepare("SELECT id, uid, creator_id, created_ts, updated_ts, filename, type, size, memo_id, storage_type, reference, payload FROM attachment ORDER BY created_ts, id").all().await?.results::<Value>()?;
    let relations = db(env)?
        .prepare("SELECT * FROM memo_relation ORDER BY memo_id, related_memo_id")
        .all()
        .await?
        .results::<Value>()?;
    Ok(json!({
        "exportedAt": js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default(),
        "users": users,
        "memos": memos,
        "attachments": attachments,
        "relations": relations
    }))
}

pub(crate) fn backup_preview(payload: &Value) -> Value {
    json!({
        "userCount": payload.get("users").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "memoCount": payload.get("memos").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "attachmentCount": payload.get("attachments").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "relationCount": payload.get("relations").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
    })
}

fn backup_key() -> String {
    let stamp = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| unix_now().to_string())
        .replace([':', '.'], "-");
    format!("backups/memos-{}.json", stamp)
}

pub(crate) async fn backup_storage_body(
    env: &Env,
    plaintext: &str,
) -> std::result::Result<BackupStorageBody, AppError> {
    if let Some(key) = current_backup_encryption_key(env) {
        let text = encrypt_backup_text(&key, plaintext).await?;
        Ok(BackupStorageBody {
            text,
            encrypted: true,
            key_id: Some(key.id),
        })
    } else {
        Ok(BackupStorageBody {
            text: plaintext.to_string(),
            encrypted: false,
            key_id: None,
        })
    }
}

pub(crate) async fn backup_plaintext(
    env: &Env,
    stored: &str,
) -> std::result::Result<String, AppError> {
    let parsed = serde_json::from_str::<Value>(stored)
        .map_err(|_| AppError::new(400, "Invalid backup payload"))?;
    if parsed.get("encrypted").and_then(Value::as_bool) != Some(true) {
        return Ok(stored.to_string());
    }
    decrypt_backup_text_with_keyring(env, &parsed).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackupStorageBody {
    pub(crate) text: String,
    pub(crate) encrypted: bool,
    pub(crate) key_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackupEncryptionKey {
    pub(crate) id: String,
    pub(crate) secret: String,
}

pub(crate) fn backup_encryption_status(env: &Env) -> Value {
    let current = current_backup_encryption_key(env);
    let keys = backup_encryption_keys(env);
    json!({
        "configured": current.is_some(),
        "currentKeyId": current.as_ref().map(|key| key.id.clone()),
        "knownKeyIds": keys.into_iter().map(|key| key.id).collect::<Vec<_>>()
    })
}

pub(crate) fn current_backup_encryption_key(env: &Env) -> Option<BackupEncryptionKey> {
    env_text(env, "BACKUP_ENCRYPTION_KEY_CURRENT")
        .filter(|value| !value.trim().is_empty())
        .map(|secret| BackupEncryptionKey {
            id: env_text(env, "BACKUP_ENCRYPTION_KEY_ID")
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "current".to_string()),
            secret,
        })
        .or_else(|| {
            env_text(env, "BACKUP_ENCRYPTION_KEY")
                .filter(|value| !value.trim().is_empty())
                .map(|secret| BackupEncryptionKey {
                    id: env_text(env, "BACKUP_ENCRYPTION_KEY_ID")
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "legacy".to_string()),
                    secret,
                })
        })
}

pub(crate) fn backup_encryption_keys(env: &Env) -> Vec<BackupEncryptionKey> {
    let mut keys = Vec::new();
    if let Some(current) = current_backup_encryption_key(env) {
        keys.push(current);
    }
    if let Some(raw) = env_text(env, "BACKUP_ENCRYPTION_KEYS") {
        keys.extend(parse_backup_encryption_keys(&raw));
    }
    let mut seen = BTreeSet::new();
    keys.into_iter()
        .filter(|key| !key.id.trim().is_empty() && !key.secret.trim().is_empty())
        .filter(|key| seen.insert(key.id.clone()))
        .collect()
}

pub(crate) fn parse_backup_encryption_keys(raw: &str) -> Vec<BackupEncryptionKey> {
    if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(raw) {
        return map
            .into_iter()
            .filter_map(|(id, value)| {
                let secret = value.as_str()?.trim().to_string();
                if id.trim().is_empty() || secret.is_empty() {
                    None
                } else {
                    Some(BackupEncryptionKey { id, secret })
                }
            })
            .collect();
    }
    raw.split(',')
        .filter_map(|entry| {
            let (id, secret) = entry.split_once('=')?;
            let id = id.trim();
            let secret = secret.trim();
            if id.is_empty() || secret.is_empty() {
                None
            } else {
                Some(BackupEncryptionKey {
                    id: id.to_string(),
                    secret: secret.to_string(),
                })
            }
        })
        .collect()
}

pub(crate) async fn encrypt_backup_text(
    key: &BackupEncryptionKey,
    plaintext: &str,
) -> std::result::Result<String, AppError> {
    let iv = random_bytes(12)?;
    let ciphertext = aes_gcm_crypt(&key.secret, &iv, plaintext.as_bytes(), true).await?;
    serde_json::to_string_pretty(&json!({
        "encrypted": true,
        "version": 1,
        "algorithm": "AES-256-GCM",
        "keyId": key.id,
        "iv": base64url(&iv),
        "data": base64url(&ciphertext)
    }))
    .map_err(|error| AppError::new(500, error.to_string()))
}

pub(crate) async fn decrypt_backup_text_with_keyring(
    env: &Env,
    envelope: &Value,
) -> std::result::Result<String, AppError> {
    let keys = backup_encryption_keys(env);
    if keys.is_empty() {
        return Err(AppError::new(500, "BACKUP_ENCRYPTION_KEY is required"));
    }
    let key_id = envelope.get("keyId").and_then(Value::as_str);
    let mut candidates: Vec<BackupEncryptionKey> = if let Some(key_id) = key_id {
        keys.iter()
            .filter(|key| key.id == key_id)
            .cloned()
            .collect()
    } else {
        keys.clone()
    };
    if candidates.is_empty() {
        return Err(AppError::new(400, "Backup encryption key is unavailable"));
    }
    let mut last_error = AppError::new(400, "Backup decryption failed");
    for key in candidates.drain(..) {
        match decrypt_backup_text(&key.secret, envelope).await {
            Ok(text) => return Ok(text),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

pub(crate) async fn decrypt_backup_text(
    secret: &str,
    envelope: &Value,
) -> std::result::Result<String, AppError> {
    if envelope.get("algorithm").and_then(Value::as_str) != Some("AES-256-GCM") {
        return Err(AppError::new(400, "Unsupported backup encryption"));
    }
    let iv = envelope
        .get("iv")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Invalid encrypted backup"))?;
    let data = envelope
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::new(400, "Invalid encrypted backup"))?;
    let iv = URL_SAFE_NO_PAD
        .decode(iv)
        .map_err(|_| AppError::new(400, "Invalid encrypted backup"))?;
    let data = URL_SAFE_NO_PAD
        .decode(data)
        .map_err(|_| AppError::new(400, "Invalid encrypted backup"))?;
    let plaintext = aes_gcm_crypt(secret, &iv, &data, false).await?;
    String::from_utf8(plaintext).map_err(|_| AppError::new(400, "Invalid backup payload"))
}

async fn aes_gcm_crypt(
    secret: &str,
    iv: &[u8],
    input: &[u8],
    encrypt: bool,
) -> std::result::Result<Vec<u8>, AppError> {
    let crypto = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .map_err(|_| AppError::new(500, "Crypto API is unavailable"))?;
    let subtle = js_sys::Reflect::get(&crypto, &JsValue::from_str("subtle"))
        .map_err(|_| AppError::new(500, "Crypto API is unavailable"))?;

    let key_bytes = Sha256::digest(secret.as_bytes());
    let algorithm = js_sys::Object::new();
    js_sys::Reflect::set(
        &algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("AES-GCM"),
    )
    .map_err(|_| AppError::new(500, "Backup encryption failed"))?;
    let usages = js_sys::Array::new();
    usages.push(&JsValue::from_str("encrypt"));
    usages.push(&JsValue::from_str("decrypt"));
    let import_key = js_sys::Reflect::get(&subtle, &JsValue::from_str("importKey"))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
        .map_err(|_| AppError::new(500, "Crypto API is unavailable"))?;
    let key_array = js_sys::Uint8Array::from(key_bytes.as_slice());
    let key_promise = import_key
        .call5(
            &subtle,
            &JsValue::from_str("raw"),
            &key_array.into(),
            &algorithm.into(),
            &JsValue::FALSE,
            &usages.into(),
        )
        .map_err(|_| AppError::new(500, "Backup encryption failed"))?;
    let key = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(key_promise))
        .await
        .map_err(|_| AppError::new(500, "Backup encryption failed"))?;

    let crypt_algorithm = js_sys::Object::new();
    js_sys::Reflect::set(
        &crypt_algorithm,
        &JsValue::from_str("name"),
        &JsValue::from_str("AES-GCM"),
    )
    .map_err(|_| AppError::new(500, "Backup encryption failed"))?;
    let iv_array = js_sys::Uint8Array::from(iv);
    js_sys::Reflect::set(&crypt_algorithm, &JsValue::from_str("iv"), &iv_array.into())
        .map_err(|_| AppError::new(500, "Backup encryption failed"))?;
    let crypt_name = if encrypt { "encrypt" } else { "decrypt" };
    let crypt = js_sys::Reflect::get(&subtle, &JsValue::from_str(crypt_name))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
        .map_err(|_| AppError::new(500, "Crypto API is unavailable"))?;
    let input_array = js_sys::Uint8Array::from(input);
    let promise = crypt
        .call3(&subtle, &crypt_algorithm.into(), &key, &input_array.into())
        .map_err(|_| AppError::new(400, "Backup decryption failed"))?;
    let buffer = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|_| AppError::new(400, "Backup decryption failed"))?;
    let output = js_sys::Uint8Array::new(&buffer);
    let mut bytes = vec![0u8; output.length() as usize];
    output.copy_to(&mut bytes);
    Ok(bytes)
}
