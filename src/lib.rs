use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use futures_channel::mpsc;
use futures_util::StreamExt;
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use url::Url;
use wasm_bindgen::{JsCast, JsValue};
use worker::d1::D1Database;
use worker::*;

type HmacSha256 = Hmac<Sha256>;

const ACCESS_COOKIE: &str = "memos_access";
const REFRESH_COOKIE: &str = "memos_refresh";
const ACCESS_TTL: i64 = 15 * 60;
const REFRESH_TTL: i64 = 30 * 24 * 60 * 60;
const MIGRATION_PAGE_SIZE: usize = 100;
const MIGRATION_MAX_MEMOS: usize = 10000;
const SQL_IN_CHUNK_SIZE: usize = 50;
const MEMO_EVENT_RETENTION_DAYS: i64 = 7;

mod models;
pub(crate) use models::*;
mod routes;
pub(crate) use routes::*;
mod route_authed;
pub(crate) use route_authed::*;
mod route_public;
pub(crate) use route_public::*;
mod auth;
pub(crate) use auth::*;
mod memo_crud;
pub(crate) use memo_crud::*;
mod memo_bulk;
pub(crate) use memo_bulk::*;
mod data_endpoints;
pub(crate) use data_endpoints::*;
mod usememos_api;
pub(crate) use usememos_api::*;
mod usememos_pipeline;
pub(crate) use usememos_pipeline::*;
mod sse;
pub(crate) use sse::*;
mod memo_events;
pub(crate) use memo_events::*;
mod migration_audit;
pub(crate) use migration_audit::*;
mod shares;
pub(crate) use shares::*;
mod ai_settings;
pub(crate) use ai_settings::*;
mod backups;
pub(crate) use backups::*;
mod audit_logs;
pub(crate) use audit_logs::*;
mod webhooks;
pub(crate) use webhooks::*;
mod webhook_deliveries;
pub(crate) use webhook_deliveries::*;
mod inbox;
pub(crate) use inbox::*;
mod attachments;
pub(crate) use attachments::*;
mod comments;
pub(crate) use comments::*;
mod relation_suggestions;
pub(crate) use relation_suggestions::*;
mod relations;
pub(crate) use relations::*;
mod users;
pub(crate) use users::*;
mod user_settings;
pub(crate) use user_settings::*;
mod access_tokens;
pub(crate) use access_tokens::*;
mod tags;
pub(crate) use tags::*;
mod store;
pub(crate) use store::*;
mod domain;
pub(crate) use domain::*;
mod assets;
pub(crate) use assets::*;
mod cookies;
pub(crate) use cookies::*;
mod crypto;
pub(crate) use crypto::*;
mod http_support;
pub(crate) use http_support::*;
mod support;
pub(crate) use support::*;

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    match route(&mut req, &env).await {
        Ok(response) => Ok(response),
        Err(error) => json_response(json!({ "error": error.message }), error.status),
    }
}

#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    match create_scheduled_backup(&env).await {
        Ok(artifact) => console_log!("scheduled backup created: {}", artifact.key),
        Err(error) => console_log!("scheduled backup failed: {}", error.message),
    }
    match prune_memo_events(&env, MEMO_EVENT_RETENTION_DAYS).await {
        Ok(deleted) => console_log!("memo event prune completed: {}", deleted),
        Err(error) => console_log!("memo event prune failed: {}", error.message),
    }
}

#[cfg(test)]
mod tests;
