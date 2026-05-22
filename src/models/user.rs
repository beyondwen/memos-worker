use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicUser {
    pub(crate) id: i64,
    pub(crate) username: String,
    pub(crate) role: String,
    pub(crate) nickname: String,
    pub(crate) email: String,
    pub(crate) avatar_url: String,
    pub(crate) description: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbUser {
    pub(crate) id: i64,
    pub(crate) username: String,
    pub(crate) role: String,
    pub(crate) email: String,
    pub(crate) nickname: String,
    pub(crate) password_hash: String,
    pub(crate) avatar_url: String,
    pub(crate) description: String,
    pub(crate) row_status: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct DbAccessToken {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) token_prefix: String,
    pub(crate) created_ts: i64,
    pub(crate) updated_ts: i64,
    pub(crate) last_used_ts: Option<i64>,
    pub(crate) expires_ts: Option<i64>,
    pub(crate) row_status: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Claims {
    pub(crate) iss: String,
    pub(crate) sub: String,
    pub(crate) exp: i64,
    pub(crate) iat: i64,
    #[serde(rename = "type")]
    pub(crate) token_type: String,
    pub(crate) tid: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Viewer {
    pub(crate) id: i64,
    pub(crate) role: String,
}
