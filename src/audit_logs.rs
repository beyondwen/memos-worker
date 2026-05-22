use super::*;

pub(crate) async fn list_audit_logs(
    env: &Env,
    viewer: &Viewer,
) -> std::result::Result<Response, AppError> {
    require_admin(viewer)?;
    ensure_audit_log_table(env).await?;
    let rows = db(env)?.prepare("SELECT audit_log.*, \"user\".username AS actor_username FROM audit_log LEFT JOIN \"user\" ON \"user\".id = audit_log.actor_id ORDER BY audit_log.created_ts DESC, audit_log.id DESC LIMIT 100")
        .all()
        .await?;
    let logs: Vec<DbAuditLog> = rows.results()?;
    let payload: Vec<Value> = logs
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "createdTs": row.created_ts,
                "actorId": row.actor_id,
                "actorUsername": row.actor_username,
                "action": row.action,
                "actionLabel": audit_action_label(&row.action),
                "target": row.target,
                "detail": serde_json::from_str::<Value>(&row.detail).unwrap_or_else(|_| json!({}))
            })
        })
        .collect();
    json_response(json!({ "logs": payload }), 200).map_err(AppError::from)
}
