use super::*;

#[test]
fn backup_artifact_api_payload_includes_key_and_size() {
    let artifact = BackupArtifact {
        key: "backups/memos-test.json".to_string(),
        size: 42,
        encrypted: true,
        key_id: Some("k1".to_string()),
    };

    assert_eq!(
        backup_artifact_payload(&artifact),
        json!({ "backup": { "key": "backups/memos-test.json", "size": 42, "encrypted": true, "keyId": "k1" } })
    );
}

#[test]
fn sanitize_filename_replaces_unsafe_names() {
    assert_eq!(sanitize_filename("../bad:name?.png"), ".._bad_name_.png");
    assert_eq!(sanitize_filename("\u{0000}"), "attachment");
}

#[test]
fn random_bytes_with_filler_returns_exact_length() {
    let bytes = random_bytes_with_filler(4, |output| {
        output.copy_from_slice(&[1, 2, 3, 4]);
        Ok(())
    })
    .expect("random bytes");

    assert_eq!(bytes, vec![1, 2, 3, 4]);
}

#[test]
fn attachment_storage_key_uses_creator_uid_and_filename() {
    assert_eq!(
        attachment_storage_key(7, "a_123", "note.txt"),
        "attachments/7/a_123/note.txt"
    );
}

#[test]
fn public_memo_with_attachments_preserves_attachment_payload() {
    let memo = sample_memo();
    let attachments = vec![json!({ "uid": "a_1", "filename": "note.txt" })];
    let public = public_memo_with_attachments(memo, attachments.clone());

    assert_eq!(public.attachments, attachments);
}

#[test]
fn public_memos_with_attachments_preserves_order_and_groups_by_memo() {
    let first = DbMemo {
        id: 1,
        uid: "m_1".to_string(),
        ..sample_memo()
    };
    let second = DbMemo {
        id: 2,
        uid: "m_2".to_string(),
        ..sample_memo()
    };
    let attachments = vec![
        sample_attachment(2, "a_2", "second.txt"),
        sample_attachment(1, "a_1", "first.txt"),
    ];

    let public = public_memos_with_attachments(vec![first, second], attachments);

    assert_eq!(
        public
            .iter()
            .map(|memo| memo.uid.as_str())
            .collect::<Vec<_>>(),
        vec!["m_1", "m_2"]
    );
    assert_eq!(public[0].attachments[0]["uid"], "a_1");
    assert_eq!(public[1].attachments[0]["uid"], "a_2");
}

#[test]
fn memo_list_index_migration_matches_home_query_order() {
    let migration = std::fs::read_to_string("migrations/0005_memo_list_indexes.sql")
        .expect("memo list index migration");

    assert!(migration.contains("idx_memo_list_home"));
    assert!(migration.contains("memo(row_status, pinned, created_ts, id)"));
}

#[test]
fn personal_cleanup_migration_drops_removed_feature_tables() {
    let migration = std::fs::read_to_string("migrations/0006_personal_cleanup.sql")
        .expect("personal cleanup migration");

    for table in [
        "reaction",
        "memo_share",
        "inbox",
        "webhook_delivery",
        "webhook",
    ] {
        assert!(migration.contains(&format!("DROP TABLE IF EXISTS {}", table)));
    }
}

#[test]
fn security_and_index_migration_creates_rate_limit_and_memo_index_tables() {
    let migration = std::fs::read_to_string("migrations/0007_security_and_indexes.sql")
        .expect("security/index migration");

    assert!(migration.contains("CREATE TABLE IF NOT EXISTS auth_rate_limit"));
    assert!(migration.contains("CREATE TABLE IF NOT EXISTS memo_search"));
    assert!(migration.contains("CREATE TABLE IF NOT EXISTS memo_tag"));
    assert!(migration.contains("INSERT OR REPLACE INTO memo_search"));
    assert!(migration.contains("json_valid(memo.payload)"));
    assert!(migration.contains("TRIM(json_each.value)"));
}

#[test]
fn backup_encryption_keyring_parses_json_and_comma_formats() {
    let json_keys = parse_backup_encryption_keys(r#"{"old":"secret-old","new":"secret-new"}"#);
    assert!(json_keys.contains(&BackupEncryptionKey {
        id: "old".to_string(),
        secret: "secret-old".to_string(),
    }));
    assert!(json_keys.contains(&BackupEncryptionKey {
        id: "new".to_string(),
        secret: "secret-new".to_string(),
    }));
    assert_eq!(
        parse_backup_encryption_keys("old=secret-old, new = secret-new"),
        vec![
            BackupEncryptionKey {
                id: "old".to_string(),
                secret: "secret-old".to_string(),
            },
            BackupEncryptionKey {
                id: "new".to_string(),
                secret: "secret-new".to_string(),
            },
        ]
    );
}

#[test]
fn backup_payload_validation_requires_core_arrays_and_ids() {
    let payload = json!({
        "users": [],
        "memos": [{
            "id": 1,
            "uid": "m_1",
            "creator_id": 1,
            "created_ts": 1,
            "updated_ts": 1
        }],
        "attachments": [],
        "relations": []
    });
    assert!(validate_backup_payload(&payload).is_ok());
    assert!(validate_backup_payload(&json!({ "memos": [] })).is_err());
}

#[test]
fn backup_retention_prunes_oldest_backup_keys() {
    let backups = vec![
        (
            "backups/new.json".to_string(),
            "2026-05-22T10:00:00Z".to_string(),
        ),
        (
            "backups/old.json".to_string(),
            "2026-05-20T10:00:00Z".to_string(),
        ),
        (
            "backups/mid.json".to_string(),
            "2026-05-21T10:00:00Z".to_string(),
        ),
    ];

    assert_eq!(backup_keys_to_prune(backups, 2), vec!["backups/old.json"]);
}

#[test]
fn sse_ready_payload_is_valid_event_stream() {
    let payload = sse_ready_payload(7).expect("ready payload");

    assert!(payload.starts_with("retry: 5000\n"));
    assert!(payload.contains("event: ready\n"));
    assert!(payload.contains("\"userId\":7"));
}

#[test]
fn memo_event_sse_includes_id_event_and_payload() {
    let event = DbMemoEvent {
        id: 42,
        event_type: "memo.updated".to_string(),
        name: "memos/m_1".to_string(),
        visibility: "PRIVATE".to_string(),
        creator_id: 7,
        payload: json!({ "type": "memo.updated", "name": "memos/m_1" }).to_string(),
    };
    let payload = memo_event_sse(&event).expect("memo event");

    assert!(payload.starts_with("id: 42\nevent: memo.updated\n"));
    assert!(payload.contains("\"id\":\"42\""));
    assert!(payload.contains("\"creatorId\":7"));
}

#[test]
fn sse_since_id_prefers_last_event_id() {
    let url = Url::parse("https://memos.local/api/v1/sse?since=7").expect("url");

    assert_eq!(sse_since_id(Some("42"), &url), Some(42));
    assert_eq!(sse_since_id(None, &url), Some(7));
    assert_eq!(sse_since_id(Some("bad"), &url), Some(7));
}

#[test]
fn memo_event_payload_matches_frontend_refresh_shape() {
    let memo = sample_memo();

    assert_eq!(
        memo_event_payload("memo.updated", &memo),
        json!({
            "type": "memo.updated",
            "name": "memos/m_1",
            "visibility": "PRIVATE",
            "creatorId": 7
        })
    );
}

#[test]
fn memo_event_payload_merges_detail_fields() {
    let memo = sample_memo();

    assert_eq!(
        memo_event_payload_with_detail(
            "memo.bulk.updated",
            &memo,
            json!({ "action": "ARCHIVE", "updated": 2, "deleted": 0 })
        ),
        json!({
            "type": "memo.bulk.updated",
            "name": "memos/m_1",
            "visibility": "PRIVATE",
            "creatorId": 7,
            "action": "ARCHIVE",
            "updated": 2,
            "deleted": 0
        })
    );
}

#[test]
fn memo_event_retention_cutoff_uses_whole_days() {
    assert_eq!(memo_event_retention_cutoff(1_000_000, 7), 395_200);
    assert_eq!(memo_event_retention_cutoff(1_000_000, -1), 1_000_000);
}

#[test]
fn api_worker_only_handles_backend_paths() {
    assert!(is_backend_request_path("/api/v1/memos"));
    assert!(is_backend_request_path("/file/attachments/a_1/name.png"));
    assert!(!is_backend_request_path("/"));
    assert!(!is_backend_request_path("/assets/index.js"));
    assert!(!is_backend_request_path("/memos/m_1"));
}

#[test]
fn public_signup_is_disabled_unless_env_is_truthy() {
    assert!(!is_truthy_env(None));
    assert!(!is_truthy_env(Some("false")));
    assert!(!is_truthy_env(Some("0")));
    assert!(is_truthy_env(Some("true")));
    assert!(is_truthy_env(Some("1")));
    assert!(is_truthy_env(Some(" yes ")));
}

#[test]
fn cors_allows_same_origin_or_configured_origins() {
    assert_eq!(
        allowed_cors_origin(
            Some("https://notes.example.com"),
            Some("https://notes.example.com"),
            None,
        ),
        Some("https://notes.example.com".to_string())
    );
    assert_eq!(
        allowed_cors_origin(
            Some("https://app.example.com"),
            Some("https://api.example.com"),
            Some("https://notes.example.com, https://app.example.com"),
        ),
        Some("https://app.example.com".to_string())
    );
    assert_eq!(
        allowed_cors_origin(
            Some("https://evil.example.com"),
            Some("https://api.example.com"),
            Some("https://notes.example.com"),
        ),
        None
    );
}

#[test]
fn csrf_is_required_for_cookie_authenticated_writes_only() {
    assert!(!csrf_required_for_request(&Method::Get, "memos_access=a"));
    assert!(!csrf_required_for_request(&Method::Post, ""));
    assert!(!csrf_required_for_request(&Method::Post, "other=value"));
    assert!(csrf_required_for_request(&Method::Post, "memos_access=a"));
    assert!(csrf_required_for_request(
        &Method::Delete,
        "memos_refresh=r"
    ));
}

#[test]
fn csrf_tokens_must_be_present_and_equal() {
    assert!(csrf_tokens_match(Some("csrf_abc"), Some("csrf_abc")));
    assert!(csrf_tokens_match(Some(" csrf_abc "), Some("csrf_abc")));
    assert!(!csrf_tokens_match(Some("csrf_abc"), Some("csrf_other")));
    assert!(!csrf_tokens_match(None, Some("csrf_abc")));
    assert!(!csrf_tokens_match(Some("csrf_abc"), None));
}

#[test]
fn security_policy_disallows_embeds_and_external_scripts() {
    let policy = security_content_policy();

    assert!(policy.contains("script-src 'self'"));
    assert!(policy.contains("object-src 'none'"));
    assert!(policy.contains("frame-ancestors 'none'"));
}

#[test]
fn auth_record_retention_cutoff_uses_whole_days() {
    assert_eq!(auth_record_retention_cutoff(1_000_000, 30), -1_592_000);
    assert_eq!(auth_record_retention_cutoff(1_000_000, -1), 1_000_000);
}

#[test]
fn memo_page_tokens_round_trip_cursor_fields() {
    let memo = sample_memo();
    let token = build_memo_page_token(&memo);

    assert_eq!(
        parse_memo_page_token(&token),
        Some(MemoPageCursor {
            pinned: memo.pinned,
            created_ts: memo.created_ts,
            id: memo.id,
        })
    );
    assert_eq!(parse_memo_page_token("offset-20"), None);
}

#[test]
fn memo_date_filters_build_utc_day_bounds() {
    let url = Url::parse(
        "https://memos.local/api/v1/memos?created_after=2026-05-21&created_before=2026-05-21",
    )
    .expect("url");

    assert_eq!(memo_created_after_ts(&url).unwrap(), Some(1_779_321_600));
    assert_eq!(
        memo_created_before_exclusive_ts(&url).unwrap(),
        Some(1_779_408_000)
    );
}

#[test]
fn memo_date_filters_reject_invalid_dates() {
    let url = Url::parse("https://memos.local/api/v1/memos?created_after=2026-13-01").expect("url");

    assert!(memo_created_after_ts(&url).is_err());
}

#[test]
fn memo_created_ts_from_body_accepts_unix_and_datetime_values() {
    assert_eq!(
        memo_created_ts_from_body(&json!({ "createdTs": 1_779_321_600 }), 1).unwrap(),
        1_779_321_600
    );
    assert_eq!(
        memo_created_ts_from_body(&json!({ "createdAt": "2026-05-21T00:00" }), 1).unwrap(),
        1_779_321_600
    );
    assert_eq!(memo_created_ts_from_body(&json!({}), 7).unwrap(), 7);
    assert_eq!(
        memo_created_ts_from_body(&json!({ "createdTs": null }), 7).unwrap(),
        7
    );
}

#[test]
fn memo_created_ts_from_body_rejects_invalid_values() {
    assert!(memo_created_ts_from_body(&json!({ "createdTs": -1 }), 1).is_err());
    assert!(memo_created_ts_from_body(&json!({ "createdAt": "bad" }), 1).is_err());
}

#[test]
fn memo_tags_from_payload_normalizes_unique_tags() {
    assert_eq!(
        memo_tags_from_payload(r##"{"tags":["work"," work ","","life","work"]}"##),
        vec!["work".to_string(), "life".to_string()]
    );
}

#[test]
fn calendar_month_bounds_use_utc_month_edges() {
    assert_eq!(
        calendar_month_bounds(2026, 5).expect("month bounds"),
        (1_777_593_600, 1_780_272_000)
    );
    assert!(calendar_month_bounds(1969, 12).is_err());
    assert!(calendar_month_bounds(2026, 13).is_err());
}

#[test]
fn calendar_country_codes_are_two_uppercase_letters() {
    assert!(valid_country_code("US"));
    assert!(valid_country_code("CN"));
    assert!(!valid_country_code("usa"));
    assert!(!valid_country_code("U1"));
}

#[test]
fn memo_index_health_json_marks_drift() {
    let health = MemoIndexHealth {
        memo_count: 2,
        search_count: 1,
        missing_search_count: 1,
        orphan_search_count: 0,
        tag_count: 3,
        orphan_tag_count: 0,
        healthy: false,
    };

    assert_eq!(health.to_json()["healthy"], false);
    assert_eq!(health.to_json()["missingSearchCount"], 1);
}

#[test]
fn parse_user_settings_path_splits_identifier_and_key() {
    assert_eq!(
        parse_user_settings_path("alice/settings/theme"),
        Some(("alice", Some("theme")))
    );
    assert_eq!(
        parse_user_settings_path("alice/settings"),
        Some(("alice", None))
    );
    assert_eq!(parse_user_settings_path("alice"), None);
}

#[test]
fn memo_child_routes_classify_unknown_paths_as_unsupported() {
    assert_eq!(
        memo_child_route(&["m_1", "relations", "suggest"], &Method::Post),
        MemoChildRoute::SuggestRelations
    );
    assert_eq!(
        memo_child_route(&["m_1", "relations", "suggest"], &Method::Get),
        MemoChildRoute::Unsupported
    );
    assert_eq!(
        memo_child_route(&["m_1", "unknown"], &Method::Get),
        MemoChildRoute::Unsupported
    );
    assert_eq!(
        memo_child_route(&["m_1", "reactions"], &Method::Get),
        MemoChildRoute::Unsupported
    );
    assert_eq!(
        memo_child_route(&["m_1", "shares"], &Method::Post),
        MemoChildRoute::Unsupported
    );
}

#[test]
fn relation_candidates_rank_shared_tags_and_keywords_first() {
    let current = RelationCandidate {
        uid: "m_current".to_string(),
        content: "今天研究 Memos 迁移，想把导入的数据做成知识图谱".to_string(),
        payload: json!({ "tags": ["memos", "graph"] }).to_string(),
        updated_ts: 2000,
    };
    let mut candidates = vec![RelationCandidate {
        uid: "m_related".to_string(),
        content: "Memos 导入之后可以通过引用关系形成 graph".to_string(),
        payload: json!({ "tags": ["memos"] }).to_string(),
        updated_ts: 1000,
    }];
    candidates.extend((0..60).map(|index| RelationCandidate {
        uid: format!("m_{}", index),
        content: format!("普通日记 {}", index),
        payload: "{}".to_string(),
        updated_ts: 900 - index,
    }));

    let ranked = rank_relation_candidates(&current, &candidates, 30);

    assert_eq!(ranked.len(), 30);
    assert_eq!(ranked[0].uid, "m_related");
    assert!(ranked[0].score > ranked[1].score);
}

#[test]
fn parse_ai_relation_suggestions_drops_unknown_memos() {
    let mut candidates = HashMap::new();
    candidates.insert("m_related".to_string(), "content".to_string());
    let parsed = parse_ai_relation_suggestions(
        &json!({
            "suggestions": [
                { "memo": "memos/m_related", "reason": "都在讨论 Memos 迁移", "confidence": 0.82 },
                { "memo": "memos/m_missing", "reason": "不存在", "confidence": 0.9 }
            ]
        })
        .to_string(),
        &candidates,
    );

    assert_eq!(
        parsed,
        vec![json!({
            "memo": "memos/m_related",
            "content": "content",
            "reason": "都在讨论 Memos 迁移",
            "confidence": 0.82,
            "source": "ai"
        })]
    );
}

#[test]
fn requested_relation_uids_normalizes_deduplicates_and_skips_self() {
    let body = json!({
        "relations": [
            { "memo": "memos/m_related" },
            { "memo": " https://notes.local/memos/m_related " },
            { "memo": "#/memos/m_hash" },
            { "memo": "m_current" },
            { "memo": "" },
            { "memo": "m_other" }
        ]
    });

    assert_eq!(
        requested_relation_uids(&body, "m_current"),
        vec![
            "m_related".to_string(),
            "m_hash".to_string(),
            "m_other".to_string()
        ]
    );
}

#[test]
fn relation_suggestions_payload_includes_ai_fallback_warning() {
    let suggestions = vec![json!({
        "memo": "memos/m_local",
        "content": "local",
        "reason": "标签或关键词相近",
        "confidence": 0.6,
        "source": "local"
    })];

    let payload = relation_suggestions_payload(
        suggestions.clone(),
        RelationSuggestionSource::LocalFallback {
            warning: Some("AI API returned HTTP 502".to_string()),
        },
    );

    assert_eq!(payload["suggestions"], json!(suggestions));
    assert_eq!(payload["source"], "local");
    assert_eq!(payload["warning"], "AI API returned HTTP 502");
}

fn sample_memo() -> DbMemo {
    DbMemo {
        id: 1,
        uid: "m_1".to_string(),
        creator_id: 7,
        creator_username: "alice".to_string(),
        creator_nickname: "Alice".to_string(),
        created_ts: 10,
        updated_ts: 11,
        row_status: "NORMAL".to_string(),
        content: "hello".to_string(),
        visibility: "PRIVATE".to_string(),
        pinned: 0,
        payload: "{}".to_string(),
    }
}

fn sample_attachment(memo_id: i64, uid: &str, filename: &str) -> DbAttachment {
    DbAttachment {
        id: memo_id,
        uid: uid.to_string(),
        creator_id: 7,
        created_ts: 10 + memo_id,
        filename: filename.to_string(),
        file_type: "text/plain".to_string(),
        size: 12,
        memo_id: Some(memo_id),
        reference: format!("attachments/7/{}/{}", uid, filename),
        memo_visibility: Some("PRIVATE".to_string()),
        memo_creator_id: Some(7),
    }
}
