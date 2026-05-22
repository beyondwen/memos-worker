use super::*;

pub(crate) fn server_secret(env: &Env) -> std::result::Result<String, AppError> {
    if let Ok(secret) = env.secret("SERVER_SECRET") {
        return Ok(secret.to_string());
    }
    env.var("SERVER_SECRET")
        .map(|value| value.to_string())
        .map_err(|_| AppError::new(500, "SERVER_SECRET is not configured"))
}

pub(crate) fn hash_password(password: &str) -> std::result::Result<String, AppError> {
    let salt = random_bytes(16)?;
    let mut derived = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, 100_000, &mut derived);
    Ok(format!(
        "pbkdf2_sha256$100000${}${}",
        base64url(&salt),
        base64url(&derived)
    ))
}

pub(crate) fn verify_password(password: &str, stored: &str) -> bool {
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

pub(crate) fn sign_jwt(claims: &Claims, secret: &str) -> std::result::Result<String, AppError> {
    let header = base64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = base64url(
        serde_json::to_string(claims)
            .map_err(|_| AppError::new(500, "JWT encode failed"))?
            .as_bytes(),
    );
    let data = format!("{}.{}", header, payload);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::new(500, "JWT key failed"))?;
    mac.update(data.as_bytes());
    let signature = mac.finalize().into_bytes();
    Ok(format!("{}.{}", data, base64url(&signature)))
}

pub(crate) fn verify_jwt(token: &str, secret: &str) -> std::result::Result<Claims, AppError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(AppError::new(401, "Invalid token"));
    }
    let data = format!("{}.{}", parts[0], parts[1]);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::new(500, "JWT key failed"))?;
    mac.update(data.as_bytes());
    let expected = mac.finalize().into_bytes();
    let actual = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| AppError::new(401, "Invalid token"))?;
    if !constant_time_equal(&actual, &expected) {
        return Err(AppError::new(401, "Invalid token"));
    }
    let payload = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| AppError::new(401, "Invalid token"))?;
    let claims: Claims =
        serde_json::from_slice(&payload).map_err(|_| AppError::new(401, "Invalid token"))?;
    if claims.iss != "memos-worker" || claims.exp < unix_now() {
        return Err(AppError::new(401, "Invalid token"));
    }
    Ok(claims)
}

pub(crate) fn base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub(crate) fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

pub(crate) fn random_bytes_with_filler<F>(
    len: usize,
    mut fill: F,
) -> std::result::Result<Vec<u8>, AppError>
where
    F: FnMut(&mut [u8]) -> std::result::Result<(), AppError>,
{
    let mut bytes = vec![0u8; len];
    fill(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn random_bytes(len: usize) -> std::result::Result<Vec<u8>, AppError> {
    random_bytes_with_filler(len, fill_crypto_random_bytes)
}

fn fill_crypto_random_bytes(output: &mut [u8]) -> std::result::Result<(), AppError> {
    let crypto = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .map_err(|_| AppError::new(500, "Crypto API is unavailable"))?;
    if crypto.is_undefined() || crypto.is_null() {
        return Err(AppError::new(500, "Crypto API is unavailable"));
    }
    let get_random_values = js_sys::Reflect::get(&crypto, &JsValue::from_str("getRandomValues"))
        .map_err(|_| AppError::new(500, "Crypto API is unavailable"))?;
    let get_random_values = get_random_values
        .dyn_into::<js_sys::Function>()
        .map_err(|_| AppError::new(500, "Crypto API is unavailable"))?;
    let array = js_sys::Uint8Array::new_with_length(output.len() as u32);
    get_random_values
        .call1(&crypto, &array)
        .map_err(|_| AppError::new(500, "Secure random generation failed"))?;
    array.copy_to(output);
    Ok(())
}

pub(crate) fn generate_uid(prefix: &str) -> std::result::Result<String, AppError> {
    Ok(format!("{}_{}", prefix, base64url(&random_bytes(12)?)))
}
