use super::*;

pub(crate) async fn authed_route(
    req: &mut Request,
    env: &Env,
    url: &Url,
    path: &str,
    method: Method,
    viewer: Viewer,
) -> std::result::Result<Response, AppError> {
    if path == "/api/v1/auth/user" && method == Method::Get {
        let user = get_user_by_id(env, viewer.id)
            .await?
            .ok_or_else(|| AppError::new(401, "User unavailable"))?;
        return json_response(json!({ "user": public_user(user) }), 200).map_err(AppError::from);
    }
    if path == "/api/v1/users/me" && method == Method::Patch {
        return update_me(req, env, &viewer).await;
    }
    if path == "/api/v1/auth/change-password" && method == Method::Post {
        return change_password(req, env, &viewer).await;
    }
    if path == "/api/v1/auth/sessions" && method == Method::Get {
        return list_sessions(req, env, &viewer).await;
    }
    if path.starts_with("/api/v1/auth/sessions/") && method == Method::Delete {
        let session_id = path.trim_start_matches("/api/v1/auth/sessions/");
        return revoke_session_route(env, &viewer, session_id).await;
    }
    if path == "/api/v1/users" && method == Method::Get {
        return list_users(env, &viewer).await;
    }
    if path == "/api/v1/memos" && method == Method::Get {
        return list_memos(env, url, &viewer).await;
    }
    if path == "/api/v1/memos" && method == Method::Post {
        return create_memo(req, env, &viewer).await;
    }
    if path == "/api/v1/memos/batch" && method == Method::Post {
        return bulk_memos(req, env, &viewer).await;
    }
    if path == "/api/v1/export/memos" && method == Method::Get {
        return export_data(env, &viewer).await;
    }
    if path == "/api/v1/import/memos" && method == Method::Post {
        return import_data(req, env, &viewer).await;
    }
    if path == "/api/v1/migration/memos/preview" && method == Method::Post {
        return migration_preview(req, env, &viewer).await;
    }
    if path == "/api/v1/migration/memos/import" && method == Method::Post {
        return migration_import(req, env, &viewer).await;
    }
    if path == "/api/v1/migration/memos/import-stream" && method == Method::Post {
        return migration_import_stream(req, env, &viewer).await;
    }
    if path == "/api/v1/migration/memos/backup-to-original" && method == Method::Post {
        return backup_to_original_memos(req, env, &viewer).await;
    }
    if path == "/api/v1/tags" && method == Method::Get {
        return list_tags(env, &viewer).await;
    }
    if path == "/api/v1/tags/rename" && method == Method::Post {
        return rename_tag(req, env, &viewer).await;
    }
    if path == "/api/v1/timeline" && method == Method::Get {
        return timeline(env, url, &viewer).await;
    }
    if path == "/api/v1/calendar/countries" && method == Method::Get {
        return list_calendar_countries().await;
    }
    if path == "/api/v1/calendar/holidays" && method == Method::Get {
        return list_calendar_holidays(url).await;
    }
    if path == "/api/v1/attachments" && method == Method::Get {
        return list_attachments(env, url, &viewer).await;
    }
    if path == "/api/v1/attachments" && method == Method::Post {
        return upload_attachment(req, env, &viewer).await;
    }
    if path == "/api/v1/attachments/batch-delete" && method == Method::Post {
        return batch_delete_attachments(req, env, &viewer).await;
    }
    if path.starts_with("/api/v1/attachments/") && method == Method::Delete {
        let uid = path.trim_start_matches("/api/v1/attachments/");
        return delete_attachment(env, &viewer, uid).await;
    }
    if path.starts_with("/file/attachments/") && method == Method::Get {
        let rest = path.trim_start_matches("/file/attachments/");
        let uid = rest.split('/').next().unwrap_or("");
        return download_attachment(env, &viewer, uid).await;
    }
    if path == "/api/v1/backups" && method == Method::Get {
        return list_backups(env, &viewer).await;
    }
    if path == "/api/v1/backups" && method == Method::Post {
        return create_backup(env, &viewer).await;
    }
    if path == "/api/v1/backups/download" && method == Method::Get {
        return download_backup(env, url, &viewer).await;
    }
    if path == "/api/v1/backups/preview" && method == Method::Post {
        return preview_backup(req, env, &viewer).await;
    }
    if path == "/api/v1/backups/restore" && method == Method::Post {
        return restore_backup(req, env, &viewer).await;
    }
    if path == "/api/v1/system/health" && method == Method::Get {
        return system_health(env, &viewer).await;
    }
    if path == "/api/v1/memo-index/rebuild" && method == Method::Post {
        return rebuild_memo_index_route(env, &viewer).await;
    }
    if path == "/api/v1/relations/rebuild" && method == Method::Post {
        return rebuild_relations_batch_route(req, env, &viewer).await;
    }
    if path == "/api/v1/ai/settings" && method == Method::Get {
        return get_ai_settings(env, &viewer).await;
    }
    if path == "/api/v1/ai/settings" && method == Method::Patch {
        return update_ai_settings(req, env, &viewer).await;
    }
    if path == "/api/v1/ai/settings/test" && method == Method::Post {
        return test_ai_settings(req, env, &viewer).await;
    }
    if path == "/api/v1/audit-logs" && method == Method::Get {
        return list_audit_logs(env, &viewer).await;
    }
    if path.starts_with("/api/v1/users/") && path.ends_with("/stats") && method == Method::Get {
        let identifier = path
            .trim_start_matches("/api/v1/users/")
            .trim_end_matches("/stats")
            .trim_end_matches('/');
        return user_stats(env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/")
        && path.ends_with("/access-tokens")
        && method == Method::Get
    {
        let identifier = path
            .trim_start_matches("/api/v1/users/")
            .trim_end_matches("/access-tokens")
            .trim_end_matches('/');
        return list_access_tokens(env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/")
        && path.ends_with("/access-tokens")
        && method == Method::Post
    {
        let identifier = path
            .trim_start_matches("/api/v1/users/")
            .trim_end_matches("/access-tokens")
            .trim_end_matches('/');
        return create_access_token(req, env, &viewer, identifier).await;
    }
    if path.starts_with("/api/v1/users/")
        && path.contains("/access-tokens/")
        && method == Method::Delete
    {
        let rest = path.trim_start_matches("/api/v1/users/");
        if let Some((identifier, token_id)) = rest.split_once("/access-tokens/") {
            return delete_access_token(env, &viewer, identifier, token_id).await;
        }
    }
    if path == "/api/v1/sse" && method == Method::Get {
        return connect_sse(req, env, url, &viewer).await;
    }

    if let Some(uid) = path.strip_prefix("/api/v1/memos/") {
        return memo_subroute(req, env, &viewer, uid, method, url).await;
    }
    if let Some(identifier) = path.strip_prefix("/api/v1/users/") {
        return user_subroute(req, env, &viewer, identifier, method).await;
    }

    Err(AppError::new(404, "Not found"))
}
