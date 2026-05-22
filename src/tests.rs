use super::*;

#[test]
fn backup_artifact_api_payload_includes_key_and_size() {
    let artifact = BackupArtifact {
        key: "backups/memos-test.json".to_string(),
        size: 42,
    };

    assert_eq!(
        backup_artifact_payload(&artifact),
        json!({ "backup": { "key": "backups/memos-test.json", "size": 42 } })
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
fn shared_attachment_url_uses_share_and_attachment_identity() {
    assert_eq!(
        shared_attachment_url("s_1", "a_1", "note.txt"),
        "/api/v1/shares/s_1/attachments/a_1/note.txt"
    );
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
fn memo_webhook_body_wraps_event_timestamp_and_public_memo() {
    let memo = sample_memo();

    assert_eq!(
        memo_webhook_body(
            "memo.updated",
            &memo,
            1779345600,
            json!({ "source": "test" })
        ),
        json!({
            "event": "memo.updated",
            "timestamp": 1779345600,
            "payload": {
                "memo": {
                    "name": "memos/m_1",
                    "id": 1,
                    "uid": "m_1",
                    "creator": { "id": 7, "username": "alice", "nickname": "Alice" },
                    "createdTs": 10,
                    "updatedTs": 11,
                    "rowStatus": "NORMAL",
                    "content": "hello",
                    "visibility": "PRIVATE",
                    "pinned": false,
                    "payload": {},
                    "attachments": []
                },
                "detail": { "source": "test" }
            }
        })
    );
}

#[test]
fn share_payload_includes_database_id_for_delete_route() {
    assert_eq!(
        share_payload(9, "s_1", "m_1", 1779345600, Some(1779349200)),
        json!({
            "id": 9,
            "uid": "s_1",
            "memoUid": "m_1",
            "createdTs": 1779345600,
            "expiresTs": 1779349200,
            "url": "/api/v1/shares/s_1"
        })
    );
}

#[test]
fn comment_inbox_message_points_to_parent_and_comment() {
    assert_eq!(
        comment_inbox_message("m_parent", "m_comment"),
        json!({
            "type": "memo.comment.created",
            "memoUid": "m_parent",
            "commentUid": "m_comment"
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
fn safe_inbox_message_parse_falls_back_to_unknown() {
    assert_eq!(
        safe_inbox_message("{\"type\":\"memo.comment.created\",\"memoUid\":\"m_1\"}")["type"],
        "memo.comment.created"
    );
    assert_eq!(safe_inbox_message("not json"), json!({ "type": "unknown" }));
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
