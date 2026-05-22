use super::*;

pub(crate) async fn record_migration_audit(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    progress: &MigrationProgress,
) {
    record_audit(
        env,
        Some(viewer),
        "migration.usememos.import",
        "usememos",
        json!({
            "baseUrl": options.base_url,
            "imported": progress.imported,
            "skipped": progress.skipped,
            "memoCount": progress.memo_count,
            "attachmentCount": progress.attachment_count,
            "relationCount": progress.relation_count,
            "archivedCount": progress.archived_count,
            "truncated": progress.truncated
        }),
    )
    .await;
}

pub(crate) async fn record_migration_start_audit(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
) {
    record_audit(
        env,
        Some(viewer),
        "migration.usememos.start",
        "usememos",
        json!({
            "baseUrl": options.base_url,
            "includeArchived": options.include_archived
        }),
    )
    .await;
}

pub(crate) async fn record_migration_error_audit(
    env: &Env,
    viewer: &Viewer,
    options: &MigrationOptions,
    message: &str,
) {
    record_audit(
        env,
        Some(viewer),
        "migration.usememos.error",
        "usememos",
        json!({
            "baseUrl": options.base_url,
            "error": message
        }),
    )
    .await;
}
