use super::*;

pub(crate) async fn route(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let url = req.url().map_err(AppError::from)?;
    let path = url.path().to_string();
    let method = req.method();

    if method == Method::Options {
        return Ok(empty_response(204));
    }

    if path == "/api/v1/instance" && method == Method::Get {
        return get_instance(env).await;
    }
    if path == "/api/v1/setup" && method == Method::Post {
        return setup_admin(req, env).await;
    }
    if path == "/api/v1/auth/signup" && method == Method::Post {
        return sign_up(req, env).await;
    }
    if path == "/api/v1/auth/signin" && method == Method::Post {
        return sign_in(req, env).await;
    }
    if path == "/api/v1/auth/refresh" && method == Method::Post {
        return refresh_session(req, env).await;
    }
    if path == "/api/v1/auth/signout" && method == Method::Post {
        return sign_out();
    }
    if path == "/api/v1/explore/rss.xml" && method == Method::Get {
        return generate_rss(env, None).await;
    }
    if path.starts_with("/api/v1/u/") && path.ends_with("/rss.xml") && method == Method::Get {
        let username = path
            .trim_start_matches("/api/v1/u/")
            .trim_end_matches("/rss.xml")
            .trim_end_matches('/');
        return generate_rss(env, Some(username)).await;
    }
    if path.starts_with("/api/v1/shares/") && method == Method::Get {
        let rest = path.trim_start_matches("/api/v1/shares/");
        if let Some((share_uid, attachment_rest)) = rest.split_once("/attachments/") {
            let attachment_uid = attachment_rest.split('/').next().unwrap_or("");
            return download_shared_attachment(env, share_uid, attachment_uid).await;
        }
        return public_share(env, rest).await;
    }

    if path.starts_with("/api/") || path.starts_with("/file/") {
        let viewer = current_viewer(req, env).await?;
        return authed_route(req, env, &url, &path, method, viewer).await;
    }

    fetch_asset(req, env).await
}

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
    if path == "/api/v1/tags" && method == Method::Get {
        return list_tags(env, &viewer).await;
    }
    if path == "/api/v1/tags/rename" && method == Method::Post {
        return rename_tag(req, env, &viewer).await;
    }
    if path == "/api/v1/timeline" && method == Method::Get {
        return timeline(env, &viewer).await;
    }
    if path == "/api/v1/inbox" && method == Method::Get {
        return list_inbox(env, &viewer).await;
    }
    if path == "/api/v1/inbox" && method == Method::Patch {
        return update_inbox_status(req, env, &viewer).await;
    }
    if path.starts_with("/api/v1/inbox/") && method == Method::Delete {
        let id = path.trim_start_matches("/api/v1/inbox/");
        return delete_inbox_item(env, &viewer, id).await;
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
    if path == "/api/v1/webhooks" && method == Method::Get {
        return list_webhooks(env, &viewer).await;
    }
    if path == "/api/v1/webhooks" && method == Method::Post {
        return create_webhook(req, env, &viewer).await;
    }
    if path == "/api/v1/webhooks/deliveries" && method == Method::Get {
        return list_webhook_deliveries(env, url, &viewer).await;
    }
    if path.starts_with("/api/v1/webhooks/deliveries/")
        && path.ends_with("/retry")
        && method == Method::Post
    {
        let id = path
            .trim_start_matches("/api/v1/webhooks/deliveries/")
            .trim_end_matches("/retry")
            .trim_matches('/');
        return retry_webhook_delivery(env, &viewer, id).await;
    }
    if path.starts_with("/api/v1/webhooks/") && path.ends_with("/test") && method == Method::Post {
        let id = path
            .trim_start_matches("/api/v1/webhooks/")
            .trim_end_matches("/test")
            .trim_matches('/');
        return test_webhook(env, &viewer, id).await;
    }
    if let Some(id) = path.strip_prefix("/api/v1/webhooks/") {
        return match method {
            Method::Patch => update_webhook(req, env, &viewer, id).await,
            Method::Delete => delete_webhook(env, &viewer, id).await,
            _ => Err(AppError::new(405, "Method not allowed")),
        };
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
