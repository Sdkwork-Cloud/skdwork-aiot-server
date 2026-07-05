use sdkwork_aiot_storage::{table_contract, IOT_TABLES};
use sdkwork_aiot_storage::{
    AiotCommandCreateCommand, AiotCommandRepository, AiotDeviceCreateCommand,
    AiotDeviceEventCreateCommand, AiotDeviceRepository, AiotDeviceRepositoryError,
    AiotDeviceSessionRepository, AiotDeviceTwinRepository, AiotDeviceUpdateCommand,
    AiotEventRepository, AiotOutboxWriteIntent, AiotProtocolDeadLetterIntent,
    AiotProtocolIngestUnitOfWork, AiotProtocolStorageCommand, AiotStorageAssociation,
    AiotStorageWriteKind, AiotTwinPropertyUpsertCommand, OffsetListPageParams,
};
use sdkwork_aiot_storage_sqlx::{
    initial_migration_sql, migration_catalog, schema_version, InMemorySqlStatementExecutor,
    InMemorySqlxDeviceRepository, SqlBindValue, SqlDeviceWriteOperation, SqlDialect,
    SqlProtocolIngestPlanner, SqlStatementBatch, SqlStatementExecutor, SqlStatementPlan,
    SqlTransactionFailurePolicy, SqlTransactionOutcome, SqlTransactionPlan,
    SqliteSqlxDeviceRepository, SqlxProtocolIngestUnitOfWork,
};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn sqlx_device_repository_tracks_create_update_delete_writes() {
    let repo = InMemorySqlxDeviceRepository::new();
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    let created = repo
        .create_device(
            AiotDeviceCreateCommand::new(association.clone(), "device-001", "Door", "1001")
                .with_client_id("client-001")
                .with_chip_family("esp32_s3"),
        )
        .expect("create device");
    assert_eq!(created.device_id, "device-001");

    let updated = repo
        .update_device(
            AiotDeviceUpdateCommand::new(association.clone(), "device-001")
                .with_display_name("Door Updated")
                .with_status("inactive"),
        )
        .expect("update device");
    assert_eq!(updated.display_name, "Door Updated");

    repo.delete_device(&association, "device-001")
        .expect("delete device");

    let writes = repo.writes();
    assert_eq!(writes.len(), 3);
    assert!(matches!(writes[0], SqlDeviceWriteOperation::Create(_)));
    assert!(matches!(writes[1], SqlDeviceWriteOperation::Update(_)));
    assert!(matches!(writes[2], SqlDeviceWriteOperation::Delete { .. }));

    let executed = repo.executed_statements();
    assert_eq!(executed.len(), 3);
    assert_eq!(executed[0].statement_kind, "device_create");
    assert_eq!(executed[1].statement_kind, "device_update");
    assert_eq!(executed[2].statement_kind, "device_delete");
    assert_eq!(executed[0].table, "iot_device");
    assert_eq!(executed[1].table, "iot_device");
    assert_eq!(executed[2].table, "iot_device");
    assert!(executed[0].sql.contains("INSERT INTO iot_device"));
    assert!(executed[1].sql.contains("UPDATE iot_device"));
    assert!(executed[2].sql.contains("DELETE FROM iot_device"));
}

#[test]
fn sqlx_device_repository_propagates_duplicate_and_not_found_errors() {
    let repo = InMemorySqlxDeviceRepository::new();
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    repo.create_device(AiotDeviceCreateCommand::new(
        association.clone(),
        "device-dup",
        "Dup",
        "1001",
    ))
    .expect("initial create");

    assert_eq!(
        repo.create_device(AiotDeviceCreateCommand::new(
            association.clone(),
            "device-dup",
            "Dup2",
            "1002",
        ))
        .err(),
        Some(AiotDeviceRepositoryError::DuplicateDeviceId)
    );

    assert_eq!(
        repo.delete_device(&association, "missing").err(),
        Some(AiotDeviceRepositoryError::NotFound)
    );
}

#[test]
fn sqlite_sqlx_device_repository_persists_crud_and_reopen_reads_state() {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("aiot-device-repo-{unique_suffix}.db"));
    let _ = std::fs::remove_file(&path);

    {
        let repo = SqliteSqlxDeviceRepository::open(&path).expect("open sqlite repo");
        let association = AiotStorageAssociation::tenant_org(100001, 0);
        repo.create_device(
            AiotDeviceCreateCommand::new(
                association.clone(),
                "sqlite-device-001",
                "SQLite Device",
                "1007",
            )
            .with_client_id("sqlite-client"),
        )
        .expect("create device");

        let updated = repo
            .update_device(
                AiotDeviceUpdateCommand::new(association.clone(), "sqlite-device-001")
                    .with_display_name("SQLite Device Updated")
                    .with_status("inactive")
                    .with_metadata_json(r#"{"source":"sqlite"}"#),
            )
            .expect("update device");
        assert_eq!(updated.display_name, "SQLite Device Updated");
        assert_eq!(updated.status, "inactive");
    }

    {
        let reopened = SqliteSqlxDeviceRepository::open(&path).expect("reopen sqlite repo");
        let association = AiotStorageAssociation::tenant_org(100001, 0);
        let retrieved = reopened
            .get_device(&association, "sqlite-device-001")
            .expect("retrieve after reopen");
        assert_eq!(retrieved.display_name, "SQLite Device Updated");
        assert_eq!(retrieved.status, "inactive");
        assert_eq!(
            retrieved.metadata_json.as_deref(),
            Some(r#"{"source":"sqlite"}"#)
        );

        reopened
            .delete_device(&association, "sqlite-device-001")
            .expect("delete after reopen");
        assert!(reopened
            .get_device(&association, "sqlite-device-001")
            .is_none());
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn sqlite_sqlx_command_repository_persists_create_and_list() {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("aiot-command-repo-{unique_suffix}.db"));
    let _ = std::fs::remove_file(&path);

    let repo = SqliteSqlxDeviceRepository::open(&path).expect("open sqlite repo");
    let association = AiotStorageAssociation::tenant_org(100001, 0);
    let created = repo
        .create_command(
            AiotCommandCreateCommand::new(association.clone(), "device-001", "speaker", "play")
                .with_request_payload_json(r#"{"text":"hello"}"#)
                .with_request_media(
                    Some("media-res-001".to_string()),
                    Some("obj-blob-001".to_string()),
                    Some(
                        r#"{"id":"media-res-001","kind":"audio","source":"object_storage","objectBlobId":"obj-blob-001","mimeType":"audio/opus","sizeBytes":"4096"}"#
                            .to_string(),
                    ),
                )
                .with_trace_id("trace-001"),
        )
        .expect("create command");
    assert_eq!(created.device_id, "device-001");
    assert_eq!(created.capability_name, "speaker");

    let listed = repo
        .list_commands(
            &association,
            "device-001",
            OffsetListPageParams::parse(None, None),
        )
        .expect("list commands");
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].command_name, "play");
    assert_eq!(listed.items[0].status, "accepted");
    assert_eq!(
        listed.items[0].request_media_resource_id.as_deref(),
        Some("media-res-001")
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn sqlite_sqlx_command_repository_supports_cancel_command() {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("aiot-command-cancel-{unique_suffix}.db"));
    let _ = std::fs::remove_file(&path);

    let repo = SqliteSqlxDeviceRepository::open(&path).expect("open sqlite repo");
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    repo.create_command(
        AiotCommandCreateCommand::new(association.clone(), "device-cancel-001", "speaker", "play")
            .with_command_id("cmd-cancel-001"),
    )
    .expect("create command");

    let cancelled = repo
        .cancel_command(&association, "device-cancel-001", "cmd-cancel-001")
        .expect("cancel command")
        .expect("cancelled command exists");
    assert_eq!(cancelled.status, "cancelled");

    let listed = repo
        .list_commands(
            &association,
            "device-cancel-001",
            OffsetListPageParams::parse(None, None),
        )
        .expect("list commands");
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].status, "cancelled");

    let missing = repo
        .cancel_command(&association, "device-cancel-001", "cmd-missing")
        .expect("cancel missing command");
    assert!(missing.is_none());

    let _ = std::fs::remove_file(path);
}

#[test]
fn sqlite_sqlx_command_repository_scopes_idempotency_by_tenant_and_organization() {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("aiot-command-idempotency-scope-{unique_suffix}.db"));
    let _ = std::fs::remove_file(&path);

    let repo = SqliteSqlxDeviceRepository::open(&path).expect("open sqlite repo");
    let association_a = AiotStorageAssociation::tenant_org(100001, 0);
    let association_b = AiotStorageAssociation::tenant_org(100001, 1);

    let created_a = repo
        .create_command(
            AiotCommandCreateCommand::new(
                association_a.clone(),
                "device-scope-001",
                "speaker",
                "play-a",
            )
            .with_request_payload_json(r#"{"text":"a"}"#)
            .with_idempotency_key("same-tenant-cross-org-idem-001"),
        )
        .expect("create command for organization A");
    let created_b = repo
        .create_command(
            AiotCommandCreateCommand::new(
                association_b.clone(),
                "device-scope-001",
                "speaker",
                "play-b",
            )
            .with_request_payload_json(r#"{"text":"b"}"#)
            .with_idempotency_key("same-tenant-cross-org-idem-001"),
        )
        .expect("create command for organization B");

    assert_ne!(created_a.command_id, created_b.command_id);
    assert_eq!(created_a.command_name, "play-a");
    assert_eq!(created_b.command_name, "play-b");

    let listed_a = repo
        .list_commands(
            &association_a,
            "device-scope-001",
            OffsetListPageParams::parse(None, None),
        )
        .expect("list commands organization A");
    let listed_b = repo
        .list_commands(
            &association_b,
            "device-scope-001",
            OffsetListPageParams::parse(None, None),
        )
        .expect("list commands organization B");
    assert_eq!(listed_a.items.len(), 1);
    assert_eq!(listed_b.items.len(), 1);
    assert_eq!(listed_a.items[0].command_name, "play-a");
    assert_eq!(listed_b.items[0].command_name, "play-b");

    let _ = std::fs::remove_file(path);
}

#[test]
fn sqlite_sqlx_event_and_twin_repositories_persist_and_read_snapshot() {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("aiot-event-twin-repo-{unique_suffix}.db"));
    let _ = std::fs::remove_file(&path);

    let repo = SqliteSqlxDeviceRepository::open(&path).expect("open sqlite repo");
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    repo.record_event(
        AiotDeviceEventCreateCommand::new(
            association.clone(),
            "device-001",
            "iot.device.media_frame.ingested",
        )
        .with_event_id("evt-device-001-0001")
        .with_event_version("1")
        .with_protocol("xiaozhi.websocket", "xiaozhi")
        .with_message_routing("mediaFrame", "audio", "websocket", "device_to_cloud")
        .with_message_id("msg-001")
        .with_correlation_id("corr-001")
        .with_trace_id("trace-001")
        .with_payload_hash(
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb7625e5c7c5f5a4c5d6",
        )
        .with_media(
            Some("media-res-001".to_string()),
            Some("obj-blob-001".to_string()),
            Some(
                r#"{"id":"media-res-001","kind":"audio","source":"object_storage","objectBlobId":"obj-blob-001","mimeType":"audio/opus","sizeBytes":"4096"}"#
                    .to_string(),
            ),
        )
        .with_payload_json(r#"{"codec":"opus","sampleRate":16000}"#)
        .with_occurred_at("2026-06-01T00:00:00Z"),
    )
    .expect("record event");

    let events = repo
        .list_events(
            &association,
            Some("device-001"),
            OffsetListPageParams::parse(None, None),
        )
        .expect("list events");
    assert_eq!(events.items.len(), 1);
    assert_eq!(events.items[0].event_id, "evt-device-001-0001");
    assert_eq!(events.items[0].protocol_id, "xiaozhi.websocket");
    assert_eq!(
        events.items[0].payload_hash.as_deref(),
        Some("d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb7625e5c7c5f5a4c5d6")
    );

    repo.upsert_twin_property(
        AiotTwinPropertyUpsertCommand::new(association.clone(), "device-001", "volume")
            .with_desired_value_json("80")
            .with_reported_value_json("72"),
    )
    .expect("upsert twin property");
    let twin = repo
        .get_twin_snapshot(&association, "device-001")
        .expect("get twin snapshot");
    assert_eq!(twin.desired.get("volume").map(String::as_str), Some("80"));
    assert_eq!(twin.reported.get("volume").map(String::as_str), Some("72"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn sqlite_sqlx_device_session_repository_supports_disconnect_lifecycle() {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("aiot-session-disconnect-{unique_suffix}.db"));
    let _ = std::fs::remove_file(&path);

    let repo = SqliteSqlxDeviceRepository::open(&path).expect("open sqlite repo");
    let association = AiotStorageAssociation::tenant_org(100001, 0);
    let device_id = "device-session-001";
    let session_id = "session-device-session-001-primary";

    assert!(!repo
        .is_session_disconnected(&association, device_id, session_id)
        .expect("query initial session state"));
    assert!(repo
        .disconnect_session(&association, device_id, session_id)
        .expect("disconnect first time"));
    assert!(repo
        .is_session_disconnected(&association, device_id, session_id)
        .expect("query disconnected session"));
    assert!(!repo
        .disconnect_session(&association, device_id, session_id)
        .expect("disconnect second time"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn sqlx_device_repository_rejects_non_numeric_product_id() {
    let repo = InMemorySqlxDeviceRepository::new();
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    let result = repo.create_device(AiotDeviceCreateCommand::new(
        association,
        "device-invalid-product",
        "Invalid Product",
        "product-alpha",
    ));

    assert_eq!(
        result.err(),
        Some(AiotDeviceRepositoryError::InvalidProductId)
    );
}

#[test]
fn migration_catalog_declares_standard_iot_tables() {
    let sql = initial_migration_sql();

    assert_eq!(schema_version(), "0.2.0");
    assert!(sql.contains("CREATE TABLE iot_device"));
    assert!(sql.contains("tenant_id"));
    assert!(sql.contains("organization_id"));
    assert!(sql.contains("CREATE TABLE iot_media_resource"));
    assert!(sql.contains("CREATE TABLE iot_device_media"));
    assert!(sql.contains("CREATE TABLE iot_outbox_event"));
}

#[test]
fn initial_migration_contains_core_constraints_and_no_iam_tables() {
    let sql = initial_migration_sql();

    for expected in [
        "owner_type",
        "owner_id",
        "created_by",
        "updated_by",
        "CONSTRAINT uk_iot_device_uuid",
        "CONSTRAINT uk_iot_device_tenant_device_key",
        "CREATE INDEX idx_iot_device_tenant_product_status",
        "CREATE TABLE iot_device_session",
        "CREATE TABLE iot_command",
        "UNIQUE (tenant_id, organization_id, idempotency_key)",
        "CREATE TABLE iot_device_twin_property",
        "CREATE TABLE iot_telemetry_latest",
        "CREATE TABLE iot_media_resource",
        "CREATE TABLE iot_device_media",
        "CREATE TABLE iot_firmware_artifact",
        "CREATE TABLE iot_protocol_message_dead_letter",
    ] {
        assert!(sql.contains(expected), "migration missing {expected}");
    }

    for forbidden in ["CREATE TABLE iam_", "REFERENCES iam_"] {
        assert!(
            !sql.contains(forbidden),
            "AIoT migration must not own or hard-FK IAM tables"
        );
    }
}

#[test]
fn migration_catalog_is_versioned_and_ordered() {
    let catalog = migration_catalog();

    assert_eq!(catalog[0].version, "0001");
    assert_eq!(catalog[0].name, "aiot_core_schema");
    assert_eq!(catalog[0].schema_version, schema_version());
    assert!(catalog[0].sql.contains("CREATE TABLE iot_device"));
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog[1].version, "0002");
    assert_eq!(catalog[1].name, "aiot_admin_entity_schema");
    assert!(catalog[1].sql.contains("CREATE TABLE iot_admin_entity"));
}

#[test]
fn initial_migration_declares_every_standard_iot_table() {
    let sql = initial_migration_sql();

    for table in IOT_TABLES {
        assert!(
            sql.contains(&format!("CREATE TABLE {}", table.name)),
            "initial migration missing {}",
            table.name
        );
    }
}

#[test]
fn initial_migration_matches_table_contract_common_association_columns() {
    let sql = initial_migration_sql();

    for table in IOT_TABLES {
        let contract = table_contract(table.name).expect("table contract");
        let definition = table_definition(sql, table.name);

        for column in ["tenant_id", "organization_id", "data_scope"] {
            if contract.required_columns.contains(&column) {
                assert!(
                    definition.contains(column),
                    "{} DDL missing required association column {}",
                    table.name,
                    column
                );
            }
        }
    }
}

#[test]
fn initial_migration_declares_protocol_ingest_runtime_columns_and_indexes() {
    let sql = initial_migration_sql();
    let outbox = table_definition(sql, "iot_outbox_event");

    for expected in [
        "CREATE TABLE iot_protocol_message_dead_letter",
        "protocol_id VARCHAR(128) NOT NULL",
        "adapter_id VARCHAR(128) NOT NULL",
        "reason_code VARCHAR(128) NOT NULL",
        "payload_ref VARCHAR(512)",
        "CREATE INDEX idx_iot_protocol_dead_letter_tenant_created",
        "CREATE TABLE iot_outbox_event",
        "next_attempt_at TIMESTAMP",
        "attempt_count INTEGER NOT NULL DEFAULT 0",
        "CREATE INDEX idx_iot_outbox_event_status_next_attempt",
    ] {
        assert!(sql.contains(expected), "migration missing {expected}");
    }

    assert!(
        outbox.contains("data_scope INTEGER NOT NULL DEFAULT 0"),
        "iot_outbox_event must carry data_scope for SDKWork tenant association"
    );
    assert!(outbox.contains("event_version VARCHAR(16) NOT NULL DEFAULT '1'"));
    assert!(outbox.contains("payload_hash VARCHAR(128)"));
}

#[test]
fn initial_migration_aligns_media_resource_standard_for_events_and_firmware() {
    let sql = initial_migration_sql();
    let device_event = table_definition(sql, "iot_device_event");
    let firmware = table_definition(sql, "iot_firmware_artifact");
    let media_resource = table_definition(sql, "iot_media_resource");
    let device_media = table_definition(sql, "iot_device_media");

    for expected in [
        "media_resource_id VARCHAR(128)",
        "object_blob_id VARCHAR(128)",
        "media_resource_snapshot TEXT",
    ] {
        assert!(
            device_event.contains(expected),
            "iot_device_event missing {expected}"
        );
        assert!(
            firmware.contains(expected),
            "iot_firmware_artifact missing {expected}"
        );
    }

    assert!(media_resource.contains("media_resource_id VARCHAR(128) NOT NULL"));
    assert!(media_resource.contains("kind VARCHAR(32) NOT NULL"));
    assert!(media_resource.contains("source VARCHAR(32) NOT NULL"));
    assert!(media_resource.contains("resource_snapshot TEXT"));
    assert!(sql.contains("uk_iot_media_resource_tenant_resource_id"));
    assert!(sql.contains("idx_iot_media_resource_tenant_owner"));
    assert!(sql.contains("idx_iot_media_resource_tenant_object_blob"));

    assert!(device_media.contains("media_role VARCHAR(64) NOT NULL"));
    assert!(device_media.contains("media_resource_id VARCHAR(128) NOT NULL"));
    assert!(device_media.contains("resource_snapshot TEXT"));
    assert!(device_media.contains("sort_order INTEGER NOT NULL DEFAULT 0"));
    assert!(sql.contains("idx_iot_device_media_tenant_owner_role"));
    assert!(sql.contains("idx_iot_device_media_tenant_media"));

    assert!(sql.contains("uk_iot_firmware_artifact_tenant_media_resource"));
}

#[test]
fn initial_migration_declares_protocol_ingest_idempotency_and_trace_columns() {
    let sql = initial_migration_sql();

    for expected in [
        "message_id VARCHAR(128)",
        "correlation_id VARCHAR(128)",
        "media_resource_id VARCHAR(128)",
        "object_blob_id VARCHAR(128)",
        "media_resource_snapshot TEXT",
        "idempotency_key VARCHAR(256)",
        "trace_id VARCHAR(128)",
        "CONSTRAINT uk_iot_protocol_ingest_tenant_idempotency",
        "CREATE INDEX idx_iot_protocol_ingest_tenant_message",
    ] {
        assert!(sql.contains(expected), "migration missing {expected}");
    }
}

#[test]
fn initial_migration_aligns_outbox_inbox_and_dead_letter_with_event_schema_rules() {
    let sql = initial_migration_sql();
    let inbox = table_definition(sql, "iot_inbox_event");
    let dead_letter = table_definition(sql, "iot_protocol_message_dead_letter");
    let command = table_definition(sql, "iot_command");
    let command_result = table_definition(sql, "iot_command_result");

    assert!(inbox.contains("payload_hash VARCHAR(128)"));
    assert!(inbox.contains("error_message VARCHAR(1000)"));
    assert!(inbox.contains("processed_at TIMESTAMP"));

    assert!(dead_letter.contains("payload_hash VARCHAR(128)"));

    assert!(command.contains("request_media_resource_id VARCHAR(128)"));
    assert!(command.contains("request_object_blob_id VARCHAR(128)"));
    assert!(command.contains("request_media_resource_snapshot TEXT"));

    assert!(command_result.contains("result_media_resource_id VARCHAR(128)"));
    assert!(command_result.contains("result_object_blob_id VARCHAR(128)"));
    assert!(command_result.contains("result_media_resource_snapshot TEXT"));
}

#[test]
fn sqlx_protocol_uow_builds_transactional_primary_write_and_outbox_plan() {
    let executor = InMemorySqlStatementExecutor::new();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let command = AiotProtocolStorageCommand::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "device-001",
        AiotStorageWriteKind::OpenSession,
        "iot_device_session",
    )
    .with_session_id("session-001")
    .with_message_id("msg-001")
    .with_correlation_id("corr-001")
    .with_trace_id("trace-001")
    .with_idempotency_key("idem-001")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.device.session.started",
        "device_session",
        "session-001",
        "iot.protocol.ingested",
    ));

    let receipt = uow.execute_protocol_command(&command);
    let executed = executor.executed_statements();

    assert!(receipt.accepted);
    assert!(!receipt.duplicate);
    assert_eq!(executed.len(), 3);
    assert_eq!(executed[0].statement_kind, "idempotency_guard");
    assert_eq!(executed[1].statement_kind, "primary_write");
    assert_eq!(executed[1].table, "iot_protocol_ingest_record");
    assert_eq!(executed[2].statement_kind, "outbox_write");
    assert_eq!(executed[2].table, "iot_outbox_event");
    assert!(executed[0]
        .sql
        .contains("INSERT INTO iot_protocol_ingest_record"));
    assert!(executed[1]
        .sql
        .contains("UPDATE iot_protocol_ingest_record"));
    assert!(executed[2].sql.contains("INSERT INTO iot_outbox_event"));
    assert!(executed[0].sql.contains("message_id"));
    assert!(executed[0].sql.contains("correlation_id"));
    assert!(executed[0].sql.contains("media_resource_id"));
    assert!(executed[0].sql.contains("object_blob_id"));
    assert!(executed[0].sql.contains("trace_id"));
    assert!(executed[1].sql.contains("idempotency_key"));
    assert!(executed[1].sql.contains("media_resource_snapshot"));
    assert!(executed[2].sql.contains("event_version"));
    assert!(executed[2].sql.contains("payload_hash"));
    assert_eq!(command.primary_table, "iot_device_session");
}

#[test]
fn sql_protocol_ingest_plan_never_inserts_columns_missing_from_target_ddl() {
    let command = AiotProtocolStorageCommand::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "device-001",
        AiotStorageWriteKind::OpenSession,
        "iot_device_session",
    )
    .with_session_id("session-001")
    .with_message_id("msg-001")
    .with_correlation_id("corr-001")
    .with_trace_id("trace-001")
    .with_idempotency_key("idem-001")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.device.session.started",
        "device_session",
        "session-001",
        "iot.protocol.ingested",
    ));

    let plan = SqlProtocolIngestPlanner::standard()
        .try_plan_protocol_command(&command)
        .expect("valid plan");

    let mut statements = vec![plan.guard.clone()];
    statements.extend(plan.write_batch.statements.iter().cloned());

    for statement in statements
        .iter()
        .filter(|statement| statement.sql.starts_with("INSERT INTO "))
    {
        let ddl = table_definition(initial_migration_sql(), statement.table);
        for column in insert_columns(&statement.sql) {
            assert!(
                ddl_declares_column(ddl, &column),
                "{} inserts undeclared column {} into {}",
                statement.statement_kind,
                column,
                statement.table
            );
        }
    }
}

#[test]
fn sqlx_protocol_uow_treats_duplicate_idempotency_key_as_accepted_noop() {
    let executor = InMemorySqlStatementExecutor::new();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let command = AiotProtocolStorageCommand::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "device-001",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_idempotency_key("idem-telemetry-001")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.telemetry.received",
        "device",
        "device-001",
        "iot.protocol.ingested",
    ));

    let first = uow.execute_protocol_command(&command);
    let second = uow.execute_protocol_command(&command);
    let executed = executor.executed_statements();

    assert!(first.accepted);
    assert!(!first.duplicate);
    assert!(second.accepted);
    assert!(second.duplicate);
    assert_eq!(
        executed
            .iter()
            .filter(|statement| statement.statement_kind == "primary_write")
            .count(),
        1
    );
    assert_eq!(
        executed
            .iter()
            .filter(|statement| statement.statement_kind == "outbox_write")
            .count(),
        1
    );
}

#[test]
fn sqlx_protocol_uow_records_dead_letter_statement_without_raw_payload() {
    let executor = InMemorySqlStatementExecutor::new();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let intent = AiotProtocolDeadLetterIntent::from_protocol_error(
        "xiaozhi.websocket",
        "xiaozhi",
        "xiaozhi.message_type.unsupported",
        "object-store://payloads/msg-002",
    )
    .with_device_id("device-001")
    .with_trace_id("trace-002");

    let receipt = uow.record_dead_letter(&intent);
    let executed = executor.executed_statements();

    assert!(!receipt.accepted);
    assert_eq!(
        receipt.dead_letter_reason.as_deref(),
        Some("xiaozhi.message_type.unsupported")
    );
    assert_eq!(executed.len(), 1);
    assert_eq!(executed[0].statement_kind, "dead_letter_write");
    assert_eq!(executed[0].table, "iot_protocol_message_dead_letter");
    assert!(executed[0]
        .sql
        .contains("INSERT INTO iot_protocol_message_dead_letter"));
    assert!(executed[0].sql.contains("payload_ref"));
    assert!(executed[0].sql.contains("payload_hash"));
    assert!(!executed[0].sql.contains("raw_payload"));
}

#[derive(Debug, Clone, Default)]
struct RecordingSqlStatementExecutor {
    state: Arc<Mutex<RecordingSqlStatementExecutorState>>,
}

#[derive(Debug, Default)]
struct RecordingSqlStatementExecutorState {
    claimed_keys: BTreeSet<String>,
    batches: Vec<SqlStatementBatch>,
}

impl RecordingSqlStatementExecutor {
    fn batches(&self) -> Vec<SqlStatementBatch> {
        self.state
            .lock()
            .expect("recording sql executor poisoned")
            .batches
            .clone()
    }
}

impl SqlStatementExecutor for RecordingSqlStatementExecutor {
    fn execute_idempotency_guard(&self, key: &str, statement: SqlStatementPlan) -> bool {
        let mut state = self.state.lock().expect("recording sql executor poisoned");
        state
            .batches
            .push(SqlStatementBatch::single("idempotency_guard", statement));
        state.claimed_keys.insert(key.to_string())
    }

    fn execute_batch(&self, batch: SqlStatementBatch) {
        self.state
            .lock()
            .expect("recording sql executor poisoned")
            .batches
            .push(batch);
    }
}

#[test]
fn sqlx_protocol_uow_is_generic_over_executor_port_and_ordered_batches() {
    let executor = RecordingSqlStatementExecutor::default();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let command = AiotProtocolStorageCommand::new(
        "mqtt.v5",
        "mqtt",
        "device-002",
        AiotStorageWriteKind::ApplyDesiredTwin,
        "iot_device_twin_property",
    )
    .with_message_id("msg-002")
    .with_correlation_id("corr-002")
    .with_trace_id("trace-002")
    .with_idempotency_key("idem-002")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.device.property.reported",
        "device",
        "device-002",
        "iot.protocol.ingested",
    ));

    let first = uow.execute_protocol_command(&command);
    let second = uow.execute_protocol_command(&command);
    let batches = executor.batches();

    assert!(first.accepted);
    assert!(!first.duplicate);
    assert!(second.accepted);
    assert!(second.duplicate);
    assert_eq!(batches.len(), 3);
    assert_eq!(batches[0].batch_kind, "idempotency_guard");
    assert_eq!(batches[0].statements[0].statement_kind, "idempotency_guard");
    assert_eq!(batches[1].batch_kind, "protocol_ingest_write");
    assert_eq!(
        batches[1]
            .statements
            .iter()
            .map(|statement| statement.statement_kind)
            .collect::<Vec<_>>(),
        vec!["primary_write", "outbox_write"]
    );
    assert_eq!(batches[2].batch_kind, "idempotency_guard");
    assert_eq!(batches[2].statements.len(), 1);
}

#[test]
fn sqlx_protocol_uow_dead_letter_uses_executor_batch_boundary() {
    let executor = RecordingSqlStatementExecutor::default();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let intent = AiotProtocolDeadLetterIntent::from_protocol_error(
        "mqtt.v5",
        "mqtt",
        "mqtt.topic.unsupported",
        "object-store://payloads/msg-003",
    );

    let receipt = uow.record_dead_letter(&intent);
    let batches = executor.batches();

    assert!(!receipt.accepted);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].batch_kind, "dead_letter_write");
    assert_eq!(batches[0].statements.len(), 1);
    assert_eq!(
        batches[0].statements[0].table,
        "iot_protocol_message_dead_letter"
    );
}

#[test]
fn sql_statement_plans_use_bind_values_instead_of_interpolating_runtime_values() {
    let executor = InMemorySqlStatementExecutor::new();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let suspicious_device_id = "device-' OR '1'='1";
    let command = AiotProtocolStorageCommand::new(
        "xiaozhi.websocket",
        "xiaozhi",
        suspicious_device_id,
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_message_id("msg-'quoted")
    .with_correlation_id("corr-004")
    .with_trace_id("trace-004")
    .with_idempotency_key("idem-004");

    let receipt = uow.execute_protocol_command(&command);
    let executed = executor.executed_statements();

    assert!(receipt.accepted);
    assert_eq!(executed.len(), 2);

    for statement in &executed {
        assert!(
            statement.sql.contains('?') || statement.sql.contains("$1"),
            "statement must use bind placeholders: {}",
            statement.sql
        );
        assert!(
            !statement.sql.contains(suspicious_device_id),
            "runtime value leaked into SQL text: {}",
            statement.sql
        );
        assert!(
            statement
                .binds
                .contains(&SqlBindValue::Text(suspicious_device_id.to_string())),
            "runtime value missing from bind list: {:?}",
            statement.binds
        );
    }

    assert_eq!(executed[0].statement_kind, "idempotency_guard");
    assert!(matches!(executed[0].binds[0], SqlBindValue::Text(_)));
    assert_eq!(executed[0].binds[1], SqlBindValue::Int64(0));
    assert_eq!(executed[0].binds[2], SqlBindValue::Int64(0));
    assert_eq!(executed[0].binds[3], SqlBindValue::Int64(0));
    assert_eq!(
        executed[0].binds[4],
        SqlBindValue::Text("xiaozhi.websocket".to_string())
    );
    assert_eq!(
        executed[0].binds[5],
        SqlBindValue::Text("xiaozhi".to_string())
    );
    assert_eq!(
        executed[0].binds[6],
        SqlBindValue::Text(suspicious_device_id.to_string())
    );
    assert_eq!(
        executed[0].binds[7],
        SqlBindValue::Text("msg-'quoted".to_string())
    );
    assert_eq!(
        executed[0].binds[8],
        SqlBindValue::Text("corr-004".to_string())
    );
    assert_eq!(executed[0].binds[9], SqlBindValue::Null);
    assert_eq!(executed[0].binds[10], SqlBindValue::Null);
    assert_eq!(executed[0].binds[11], SqlBindValue::Null);
    assert_eq!(
        executed[0].binds[12],
        SqlBindValue::Text("idem-004".to_string())
    );
    assert_eq!(
        executed[0].binds[13],
        SqlBindValue::Text("trace-004".to_string())
    );
    assert_eq!(executed[0].binds[14], SqlBindValue::Int64(0));
    assert!(matches!(executed[0].binds[15], SqlBindValue::Text(_)));
    assert!(matches!(executed[0].binds[16], SqlBindValue::Text(_)));

    assert_eq!(executed[1].statement_kind, "primary_write");
    assert_eq!(executed[1].binds[0], SqlBindValue::Int64(1));
    assert_eq!(executed[1].binds[1], SqlBindValue::Null);
    assert_eq!(executed[1].binds[2], SqlBindValue::Null);
    assert_eq!(executed[1].binds[3], SqlBindValue::Null);
    assert_eq!(executed[1].binds[4], SqlBindValue::Int64(0));
    assert_eq!(executed[1].binds[5], SqlBindValue::Int64(0));
    assert_eq!(executed[1].binds[6], SqlBindValue::Int64(0));
    assert_eq!(
        executed[1].binds[7],
        SqlBindValue::Text("xiaozhi.websocket".to_string())
    );
    assert_eq!(
        executed[1].binds[8],
        SqlBindValue::Text("xiaozhi".to_string())
    );
    assert_eq!(
        executed[1].binds[9],
        SqlBindValue::Text(suspicious_device_id.to_string())
    );
    assert_eq!(
        executed[1].binds.last(),
        Some(&SqlBindValue::Text("idem-004".to_string()))
    );
}

#[test]
fn sql_protocol_ingest_planner_separates_statement_generation_from_execution() {
    let planner = SqlProtocolIngestPlanner::standard();
    let command = AiotProtocolStorageCommand::new(
        "coap.lwm2m",
        "lwm2m",
        "device-005",
        AiotStorageWriteKind::KeepAlive,
        "iot_device_online_lease",
    )
    .with_message_id("msg-005")
    .with_trace_id("trace-005")
    .with_idempotency_key("idem-005");

    let ingest_plan = planner.plan_protocol_command(&command);

    assert_eq!(ingest_plan.idempotency_key, "idem-005");
    assert_eq!(ingest_plan.guard.statement_kind, "idempotency_guard");
    assert_eq!(ingest_plan.write_batch.batch_kind, "protocol_ingest_write");
    assert_eq!(ingest_plan.write_batch.statements.len(), 1);
    assert_eq!(
        ingest_plan.write_batch.statements[0].table,
        "iot_protocol_ingest_record"
    );
    assert_eq!(command.primary_table, "iot_device_online_lease");
    assert!(ingest_plan.write_batch.statements[0]
        .sql
        .contains("UPDATE iot_protocol_ingest_record"));
    assert!(ingest_plan
        .guard
        .binds
        .contains(&SqlBindValue::Text("idem-005".to_string())));
    assert!(ingest_plan.write_batch.statements[0]
        .binds
        .contains(&SqlBindValue::Text("idem-005".to_string())));

    let dead_letter = planner.plan_dead_letter(&AiotProtocolDeadLetterIntent::from_protocol_error(
        "coap.lwm2m",
        "lwm2m",
        "decode.invalid_frame",
        "object-store://payloads/msg-005",
    ));

    assert_eq!(dead_letter.batch_kind, "dead_letter_write");
    assert_eq!(dead_letter.statements.len(), 1);
    assert_eq!(
        dead_letter.statements[0].table,
        "iot_protocol_message_dead_letter"
    );
}

#[test]
fn sql_protocol_ingest_planner_renders_postgres_and_sqlite_dialects() {
    let command = AiotProtocolStorageCommand::new(
        "matter.1.2",
        "matter",
        "device-007",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_message_id("msg-007")
    .with_correlation_id("corr-007")
    .with_trace_id("trace-007")
    .with_idempotency_key("idem-007");

    let postgres = SqlProtocolIngestPlanner::standard().plan_protocol_command(&command);
    let sqlite =
        SqlProtocolIngestPlanner::for_dialect(SqlDialect::Sqlite).plan_protocol_command(&command);

    assert_eq!(
        SqlProtocolIngestPlanner::standard().dialect(),
        SqlDialect::Postgres
    );
    assert_eq!(postgres.guard.dialect, SqlDialect::Postgres);
    assert_eq!(sqlite.guard.dialect, SqlDialect::Sqlite);
    assert!(postgres.guard.sql.contains("$1, $2, $3"));
    assert!(!postgres.guard.sql.contains('?'));
    assert!(postgres.write_batch.statements[0].sql.contains("$1"));
    assert!(sqlite.guard.sql.contains("?, ?, ?"));
    assert!(!sqlite.guard.sql.contains("$1"));
    assert!(postgres.guard.sql.contains("ON CONFLICT DO NOTHING"));
    assert!(sqlite.guard.sql.contains("ON CONFLICT DO NOTHING"));
    assert_eq!(postgres.guard.binds.len(), sqlite.guard.binds.len());
    assert_eq!(&postgres.guard.binds[1..=14], &sqlite.guard.binds[1..=14]);
}

#[test]
fn sql_protocol_ingest_planner_writes_appbase_association_fields() {
    let command = AiotProtocolStorageCommand::new(
        "mqtt.v5",
        "mqtt",
        "device-008",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_association(AiotStorageAssociation::tenant_org(100001, 0).with_data_scope(7))
    .with_message_id("msg-008")
    .with_trace_id("trace-008")
    .with_idempotency_key("idem-008")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.telemetry.received",
        "device",
        "device-008",
        "iot.protocol.ingested",
    ));

    let plan = SqlProtocolIngestPlanner::standard().plan_protocol_command(&command);
    let primary = &plan.write_batch.statements[0];
    let outbox = &plan.write_batch.statements[1];

    for statement in [&plan.guard, primary, outbox] {
        assert!(
            statement.sql.contains("tenant_id"),
            "{} missing tenant_id",
            statement.statement_kind
        );
        assert!(
            statement.sql.contains("organization_id"),
            "{} missing organization_id",
            statement.statement_kind
        );
        assert!(
            statement.binds.contains(&SqlBindValue::Int64(100001)),
            "{} missing tenant bind",
            statement.statement_kind
        );
        assert!(
            statement.binds.contains(&SqlBindValue::Int64(0)),
            "{} missing organization bind",
            statement.statement_kind
        );
    }

    assert!(plan.guard.sql.contains("data_scope"));
    assert!(primary.sql.contains("data_scope"));
    assert!(outbox.sql.contains("data_scope"));
    assert!(plan.guard.binds.contains(&SqlBindValue::Int64(7)));
    assert!(primary.binds.contains(&SqlBindValue::Int64(7)));
    assert!(outbox.binds.contains(&SqlBindValue::Int64(7)));
}

#[test]
fn sql_protocol_ingest_planner_rejects_unknown_primary_tables_before_rendering_sql() {
    let command = AiotProtocolStorageCommand::new(
        "mqtt.v5",
        "mqtt",
        "device-010",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event; DROP TABLE iot_device",
    )
    .with_idempotency_key("idem-010");

    let error = SqlProtocolIngestPlanner::standard()
        .try_plan_protocol_command(&command)
        .expect_err("unknown or unsafe table must fail closed");

    assert_eq!(error.code, "storage.sql.primary_table.unsupported");
    assert_eq!(
        error.table.as_deref(),
        Some("iot_telemetry_event; DROP TABLE iot_device")
    );
}

#[test]
fn sqlx_protocol_uow_rejects_unknown_primary_tables_without_executing_sql() {
    let executor = InMemorySqlStatementExecutor::new();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let command = AiotProtocolStorageCommand::new(
        "mqtt.v5",
        "mqtt",
        "device-011",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event; DROP TABLE iot_device",
    )
    .with_idempotency_key("idem-011");

    let receipt = uow.execute_protocol_command(&command);

    assert!(!receipt.accepted);
    assert_eq!(
        receipt.dead_letter_reason.as_deref(),
        Some("storage.sql.primary_table.unsupported")
    );
    assert!(executor.executed_statements().is_empty());
}

#[derive(Debug, Clone, Default)]
struct FailingTransactionExecutor {
    state: Arc<Mutex<Vec<SqlTransactionPlan>>>,
}

impl FailingTransactionExecutor {
    fn transactions(&self) -> Vec<SqlTransactionPlan> {
        self.state
            .lock()
            .expect("failing transaction executor poisoned")
            .clone()
    }
}

impl SqlStatementExecutor for FailingTransactionExecutor {
    fn execute_idempotency_guard(&self, _key: &str, _statement: SqlStatementPlan) -> bool {
        true
    }

    fn execute_batch(&self, _batch: SqlStatementBatch) {}

    fn execute_transaction(&self, transaction: SqlTransactionPlan) -> SqlTransactionOutcome {
        self.state
            .lock()
            .expect("failing transaction executor poisoned")
            .push(transaction);
        SqlTransactionOutcome::rolled_back("storage.sql.transaction_rolled_back")
    }
}

#[test]
fn sqlx_protocol_uow_maps_transaction_rollback_to_dead_letter_receipt() {
    let executor = FailingTransactionExecutor::default();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let command = AiotProtocolStorageCommand::new(
        "mqtt.v5",
        "mqtt",
        "device-013",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_idempotency_key("idem-013");

    let receipt = uow.execute_protocol_command(&command);
    let transactions = executor.transactions();

    assert!(!receipt.accepted);
    assert!(!receipt.duplicate);
    assert_eq!(
        receipt.dead_letter_reason.as_deref(),
        Some("storage.sql.transaction_rolled_back")
    );
    assert_eq!(transactions.len(), 1);
    assert_eq!(
        transactions[0].failure_policy,
        SqlTransactionFailurePolicy::RollbackAll
    );
}

#[test]
fn sql_statement_plans_validate_placeholder_count_matches_bind_count() {
    let command = AiotProtocolStorageCommand::new(
        "mqtt.v5",
        "mqtt",
        "device-012",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_idempotency_key("idem-012")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.telemetry.received",
        "device",
        "device-012",
        "iot.protocol.ingested",
    ));

    let plan = SqlProtocolIngestPlanner::standard()
        .try_plan_protocol_command(&command)
        .expect("valid plan");

    plan.validate().expect("generated plan must be valid");
    assert_eq!(plan.guard.placeholder_count(), plan.guard.binds.len());
    for statement in &plan.write_batch.statements {
        assert_eq!(statement.placeholder_count(), statement.binds.len());
    }

    let invalid = SqlStatementPlan::new(
        "invalid",
        "iot_telemetry_event",
        "INSERT INTO iot_telemetry_event (tenant_id, organization_id) VALUES ($1, $2)",
    )
    .with_binds(vec![SqlBindValue::Int64(100001)]);
    let error = invalid
        .validate()
        .expect_err("placeholder/bind mismatch must fail");
    assert_eq!(error.code, "storage.sql.bind_count_mismatch");
}

#[test]
fn sql_statement_plan_validation_rejects_columns_missing_from_target_ddl() {
    let invalid_insert = SqlStatementPlan::new(
        "invalid_insert",
        "iot_telemetry_event",
        "INSERT INTO iot_telemetry_event (tenant_id, missing_column) VALUES ($1, $2)",
    )
    .with_binds(vec![SqlBindValue::Int64(100001), SqlBindValue::Int64(1)]);
    let insert_error = invalid_insert
        .validate()
        .expect_err("insert column missing from DDL must fail");

    assert_eq!(insert_error.code, "storage.sql.column.unsupported");
    assert_eq!(insert_error.table.as_deref(), Some("iot_telemetry_event"));
    assert_eq!(insert_error.column.as_deref(), Some("missing_column"));

    let invalid_update = SqlStatementPlan::new(
        "invalid_update",
        "iot_protocol_ingest_record",
        "UPDATE iot_protocol_ingest_record SET missing_column = $1 WHERE tenant_id = $2",
    )
    .with_binds(vec![SqlBindValue::Int64(1), SqlBindValue::Int64(100001)]);
    let update_error = invalid_update
        .validate()
        .expect_err("update column missing from DDL must fail");

    assert_eq!(update_error.code, "storage.sql.column.unsupported");
    assert_eq!(
        update_error.table.as_deref(),
        Some("iot_protocol_ingest_record")
    );
    assert_eq!(update_error.column.as_deref(), Some("missing_column"));
}

#[test]
fn sql_statement_batch_and_transaction_validation_reject_invalid_nested_statements() {
    let invalid_statement = SqlStatementPlan::new(
        "invalid_update",
        "iot_protocol_ingest_record",
        "UPDATE iot_protocol_ingest_record SET missing_column = $1 WHERE tenant_id = $2",
    )
    .with_binds(vec![SqlBindValue::Int64(1), SqlBindValue::Int64(100001)]);
    let batch = SqlStatementBatch::single("invalid_batch", invalid_statement.clone());
    let batch_error = batch
        .validate()
        .expect_err("batch validation must fail on invalid statement");

    assert_eq!(batch_error.code, "storage.sql.column.unsupported");
    assert_eq!(batch_error.column.as_deref(), Some("missing_column"));

    let transaction = SqlTransactionPlan::new(
        "invalid_transaction",
        "idem-invalid",
        idempotency_guard_for_test(),
        batch,
    );
    let transaction_error = transaction
        .validate()
        .expect_err("transaction validation must fail on invalid nested statement");

    assert_eq!(transaction_error.code, "storage.sql.column.unsupported");
    assert_eq!(
        transaction_error.statement_kind,
        Some(invalid_statement.statement_kind)
    );
}

#[test]
fn sql_protocol_dead_letter_planner_exposes_validated_safe_batch_entrypoint() {
    let intent = AiotProtocolDeadLetterIntent::from_protocol_error(
        "mqtt.v5",
        "mqtt",
        "decode.invalid_frame",
        "object-store://payloads/msg-014",
    )
    .with_device_id("device-014")
    .with_trace_id("trace-014");

    let batch = SqlProtocolIngestPlanner::standard()
        .try_plan_dead_letter(&intent)
        .expect("dead-letter batch must validate");

    assert_eq!(batch.batch_kind, "dead_letter_write");
    assert_eq!(batch.statements.len(), 1);
    batch.validate().expect("safe dead-letter batch");
}

#[test]
fn sql_protocol_dead_letter_plan_writes_appbase_association_fields() {
    let intent = AiotProtocolDeadLetterIntent::from_protocol_error(
        "mqtt.v5",
        "mqtt",
        "decode.invalid_frame",
        "object-store://payloads/msg-009",
    )
    .with_association(AiotStorageAssociation::tenant_org(100001, 0).with_data_scope(7))
    .with_device_id("device-009")
    .with_trace_id("trace-009");

    let batch = SqlProtocolIngestPlanner::standard().plan_dead_letter(&intent);
    let statement = &batch.statements[0];

    assert_eq!(batch.batch_kind, "dead_letter_write");
    assert!(statement.sql.contains("tenant_id"));
    assert!(statement.sql.contains("organization_id"));
    assert!(statement.sql.contains("data_scope"));
    assert!(statement.binds.contains(&SqlBindValue::Int64(100001)));
    assert!(statement.binds.contains(&SqlBindValue::Int64(0)));
    assert!(statement.binds.contains(&SqlBindValue::Int64(7)));
    assert!(statement
        .binds
        .contains(&SqlBindValue::Text("decode.invalid_frame".to_string())));
}

fn table_definition<'a>(sql: &'a str, table: &str) -> &'a str {
    let marker = format!("CREATE TABLE {table}");
    let start = sql
        .find(&marker)
        .unwrap_or_else(|| panic!("missing table definition for {table}"));
    let rest = &sql[start + marker.len()..];
    let end = rest.find("\nCREATE TABLE ").unwrap_or(rest.len());

    &sql[start..start + marker.len() + end]
}

fn insert_columns(sql: &str) -> Vec<String> {
    let Some(start) = sql.find('(') else {
        return Vec::new();
    };
    let Some(end) = sql[start + 1..].find(')') else {
        return Vec::new();
    };

    sql[start + 1..start + 1 + end]
        .split(',')
        .map(|column| column.trim().to_string())
        .filter(|column| !column.is_empty())
        .collect()
}

fn ddl_declares_column(ddl: &str, column: &str) -> bool {
    ddl.lines()
        .map(str::trim)
        .any(|line| line.starts_with(&format!("{column} ")))
}

fn idempotency_guard_for_test() -> SqlStatementPlan {
    SqlStatementPlan::new(
        "idempotency_guard",
        "iot_protocol_ingest_record",
        "INSERT INTO iot_protocol_ingest_record (tenant_id, organization_id, data_scope, protocol_id, adapter_id, device_id, idempotency_key, status) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT DO NOTHING",
    )
    .with_binds(vec![
        SqlBindValue::Int64(100001),
        SqlBindValue::Int64(0),
        SqlBindValue::Int64(7),
        SqlBindValue::Text("mqtt.v5".to_string()),
        SqlBindValue::Text("mqtt".to_string()),
        SqlBindValue::Text("device-014".to_string()),
        SqlBindValue::Text("idem-invalid".to_string()),
        SqlBindValue::Int64(0),
    ])
}

#[derive(Debug, Clone, Default)]
struct RecordingTransactionExecutor {
    state: Arc<Mutex<RecordingTransactionExecutorState>>,
}

#[derive(Debug, Default)]
struct RecordingTransactionExecutorState {
    claimed_keys: BTreeSet<String>,
    transactions: Vec<SqlTransactionPlan>,
}

impl RecordingTransactionExecutor {
    fn transactions(&self) -> Vec<SqlTransactionPlan> {
        self.state
            .lock()
            .expect("recording transaction executor poisoned")
            .transactions
            .clone()
    }
}

impl SqlStatementExecutor for RecordingTransactionExecutor {
    fn execute_idempotency_guard(&self, key: &str, _statement: SqlStatementPlan) -> bool {
        self.state
            .lock()
            .expect("recording transaction executor poisoned")
            .claimed_keys
            .insert(key.to_string())
    }

    fn execute_batch(&self, _batch: SqlStatementBatch) {}

    fn execute_transaction(&self, transaction: SqlTransactionPlan) -> SqlTransactionOutcome {
        let mut state = self
            .state
            .lock()
            .expect("recording transaction executor poisoned");
        let accepted = state
            .claimed_keys
            .insert(transaction.idempotency_key.clone());
        state.transactions.push(transaction);

        if accepted {
            SqlTransactionOutcome::Committed
        } else {
            SqlTransactionOutcome::Duplicate
        }
    }
}

#[test]
fn sqlx_protocol_uow_executes_protocol_ingest_as_explicit_transaction_plan() {
    let executor = RecordingTransactionExecutor::default();
    let uow = SqlxProtocolIngestUnitOfWork::new(executor.clone());
    let command = AiotProtocolStorageCommand::new(
        "matter.1.2",
        "matter",
        "device-006",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_message_id("msg-006")
    .with_trace_id("trace-006")
    .with_idempotency_key("idem-006")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.telemetry.received",
        "device",
        "device-006",
        "iot.protocol.ingested",
    ));

    let first = uow.execute_protocol_command(&command);
    let second = uow.execute_protocol_command(&command);
    let transactions = executor.transactions();

    assert!(first.accepted);
    assert!(!first.duplicate);
    assert!(second.accepted);
    assert!(second.duplicate);
    assert_eq!(transactions.len(), 2);
    assert_eq!(transactions[0].transaction_kind, "protocol_ingest");
    assert_eq!(
        transactions[0].failure_policy,
        SqlTransactionFailurePolicy::RollbackAll
    );
    assert_eq!(transactions[0].idempotency_key, "idem-006");
    assert_eq!(transactions[0].guard.statement_kind, "idempotency_guard");
    assert_eq!(
        transactions[0]
            .ordered_statements()
            .iter()
            .map(|statement| statement.statement_kind)
            .collect::<Vec<_>>(),
        vec!["idempotency_guard", "primary_write", "outbox_write"]
    );
}

#[test]
fn sqlx_pool_sql_statement_executor_persists_device_create_batch() {
    use sdkwork_aiot_storage::AiotDeviceRecord;
    use sdkwork_aiot_storage_sqlx::{SqlDeviceRepositoryPlanner, SqlxPoolSqlStatementExecutor};

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("aiot-sqlx-executor-{unique_suffix}.db"));
    let _ = std::fs::remove_file(&path);

    let executor = SqlxPoolSqlStatementExecutor::open(&path).expect("open sqlx executor");
    let record = AiotDeviceRecord {
        id: "1".to_string(),
        tenant_id: 100001,
        organization_id: 0,
        device_id: "sqlx-device-001".to_string(),
        display_name: "Sqlx Device".to_string(),
        product_id: "1008".to_string(),
        client_id: Some("sqlx-client".to_string()),
        chip_family: Some("esp32_s3".to_string()),
        status: "active".to_string(),
        metadata_json: None,
        last_seen_at: "2026-01-01T00:00:00Z".to_string(),
    };
    let batch = SqlDeviceRepositoryPlanner::with_dialect(SqlDialect::Sqlite)
        .plan_create_device(&record)
        .expect("plan device create");
    executor.execute_batch(batch);

    let repo = SqliteSqlxDeviceRepository::open(&path).expect("reopen sqlite repo");
    let association = AiotStorageAssociation::tenant_org(100001, 0);
    let loaded = repo
        .get_device(&association, "sqlx-device-001")
        .expect("device persisted");
    assert_eq!(loaded.display_name, "Sqlx Device");
    assert_eq!(loaded.client_id.as_deref(), Some("sqlx-client"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn sqlite_credential_repository_verifies_hashed_bearer_tokens() {
    use sdkwork_aiot_storage_sqlx::{
        SqliteCredentialCreateCommand, SqliteSqlxCredentialRepository,
    };

    let repository = SqliteSqlxCredentialRepository::new_in_memory().expect("credential repo");
    let association = AiotStorageAssociation::tenant_org(100001, 0);
    let created = repository
        .create_credential(SqliteCredentialCreateCommand {
            association,
            device_id: "device-auth-001".to_string(),
            credential_type: "device-bearer".to_string(),
            expires_at: None,
        })
        .expect("create credential");
    let issued_secret = created
        .issued_secret
        .expect("issued secret returned once on create");

    assert!(repository.verify_bearer_token("device-auth-001", &issued_secret));
    assert!(!repository.verify_bearer_token("device-auth-001", "wrong-token"));
    assert!(!repository.verify_bearer_token("other-device", &issued_secret));

    let listed = repository
        .list_credentials(
            &AiotStorageAssociation::tenant_org(100001, 0),
            "device-auth-001",
            OffsetListPageParams::parse(None, None),
        )
        .expect("list credentials");
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].credential_type, "device-bearer");

    repository
        .revoke_credential(
            &AiotStorageAssociation::tenant_org(100001, 0),
            "device-auth-001",
            &created.credential_id,
        )
        .expect("revoke credential");
    assert!(!repository.verify_bearer_token("device-auth-001", &issued_secret));
}

#[test]
fn shared_sqlite_memory_uri_uses_one_schema_for_device_and_credential_repositories() {
    use sdkwork_aiot_storage_sqlx::{
        SqliteCredentialCreateCommand, SqliteSqlxCredentialRepository, SqliteSqlxDeviceRepository,
        DEFAULT_SHARED_SQLITE_MEMORY_URI,
    };

    let device_repo =
        SqliteSqlxDeviceRepository::open(DEFAULT_SHARED_SQLITE_MEMORY_URI).expect("device repo");
    let credential_repo = SqliteSqlxCredentialRepository::open(DEFAULT_SHARED_SQLITE_MEMORY_URI)
        .expect("credential repo");
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    device_repo
        .create_device(AiotDeviceCreateCommand::new(
            association.clone(),
            "shared-db-device",
            "Shared DB Device",
            "1001",
        ))
        .expect("create device");

    let created = credential_repo
        .create_credential(SqliteCredentialCreateCommand {
            association,
            device_id: "shared-db-device".to_string(),
            credential_type: "device-bearer".to_string(),
            expires_at: None,
        })
        .expect("create credential");
    let issued_secret = created.issued_secret.expect("issued secret");

    assert!(credential_repo.verify_bearer_token("shared-db-device", &issued_secret));
}

#[test]
fn device_repository_open_uses_sdkwork_database_config() {
    let config = sdkwork_aiot_storage_sqlx::aiot_device_sqlite_memory_config();
    assert_eq!(config.table_prefix, "iot_");
    assert_eq!(
        config.url,
        sdkwork_aiot_storage_sqlx::DEFAULT_SHARED_SQLITE_MEMORY_URI
    );
}

#[tokio::test]
async fn device_database_memory_pool_uses_sdkwork_database_sqlx() {
    let pool = sdkwork_aiot_storage_sqlx::aiot_device_sqlite_memory_pool()
        .await
        .expect("memory pool");
    assert!(pool.as_sqlite().is_some());
}

#[test]
fn sqlite_outbox_repository_publishes_pending_events() {
    use sdkwork_aiot_service_host::{
        AiotOutboxDispatcher, AiotOutboxDispatcherConfig, StructuredLogOutboxPublisher,
    };
    use sdkwork_aiot_storage::{
        AiotOutboxWriteIntent, AiotProtocolStorageCommand, AiotStorageWriteKind,
        OutboxEventRepository,
    };
    use sdkwork_aiot_storage_sqlx::{SqliteOutboxEventRepository, SqlxPoolSqlStatementExecutor};
    use std::sync::Arc;

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let db_uri = format!("file:outbox-repo-test-{unique_suffix}?mode=memory&cache=shared");

    let executor = SqlxPoolSqlStatementExecutor::open(&db_uri).expect("sqlite executor");
    let uow = executor.protocol_ingest_unit_of_work();
    let command = AiotProtocolStorageCommand::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "device-outbox-001",
        AiotStorageWriteKind::OpenSession,
        "iot_device_session",
    )
    .with_session_id("session-outbox-001")
    .with_idempotency_key("outbox-idem-001")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.device.session.started",
        "device_session",
        "session-outbox-001",
        "iot.protocol.ingested",
    ));
    let receipt = uow.execute_protocol_command(&command);
    assert!(
        receipt.accepted,
        "protocol ingest failed: {:?}",
        receipt.dead_letter_reason
    );

    let repo = SqliteOutboxEventRepository::open(&db_uri).expect("outbox repo");
    assert_eq!(repo.pending_lag_count(), 1);

    let dispatcher = AiotOutboxDispatcher::new(
        Arc::new(repo),
        Arc::new(StructuredLogOutboxPublisher),
        AiotOutboxDispatcherConfig::default(),
    );
    assert_eq!(dispatcher.run_once(), 1);
    assert_eq!(dispatcher.pending_lag_count(), 0);
}
