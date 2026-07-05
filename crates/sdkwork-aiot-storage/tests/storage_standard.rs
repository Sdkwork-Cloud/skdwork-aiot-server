use sdkwork_aiot_storage::{
    standard_dead_letter_reason_catalog, standard_protocol_ingest_storage_ports, table_contract,
    AiotCommandCreateCommand, AiotCommandRepository, AiotDeviceCreateCommand,
    AiotDeviceEventCreateCommand, AiotDeviceRepository, AiotDeviceRepositoryError,
    AiotDeviceSessionRepository, AiotDeviceTwinRepository, AiotDeviceUpdateCommand,
    AiotEventRepository, AiotOutboxWriteIntent, AiotProtocolDeadLetterIntent,
    AiotProtocolIngestUnitOfWork, AiotProtocolStorageCommand, AiotRetryPolicy,
    AiotStorageAssociation, AiotStorageFailure, AiotStorageFailureDisposition,
    AiotStorageWriteKind, AiotStorageWriteReceipt, AiotTable, AiotTwinPropertyUpsertCommand,
    InMemoryAiotCommandRepository, InMemoryAiotDeviceRepository,
    InMemoryAiotDeviceSessionRepository, InMemoryAiotDeviceTwinRepository,
    InMemoryAiotEventRepository, InMemoryProtocolIngestUnitOfWork, OffsetListPageParams,
    TableProfile, IOT_DATABASE_PREFIX, IOT_TABLES,
};

#[test]
fn table_catalog_uses_iot_prefix_and_declares_core_groups() {
    assert!(IOT_TABLES
        .iter()
        .all(|table| table.name.starts_with("iot_")));
    assert!(IOT_TABLES.contains(&AiotTable::new("iot_device", "device_registry")));
    assert!(IOT_TABLES.contains(&AiotTable::new("iot_command", "command_control")));
    assert!(IOT_TABLES.contains(&AiotTable::new("iot_media_resource", "media_resource")));
    assert!(IOT_TABLES.contains(&AiotTable::new("iot_device_media", "media_resource")));
    assert!(IOT_TABLES.contains(&AiotTable::new("iot_outbox_event", "eventing")));
}

#[test]
fn table_contract_declares_sdkwork_database_boundary_rules() {
    assert_eq!(IOT_DATABASE_PREFIX, "iot");

    let device = table_contract("iot_device").expect("iot_device contract");
    assert_eq!(device.profile, TableProfile::TenantOwnerEntity);
    assert_eq!(device.write_owner, "sdkwork-iot-device-service");
    assert!(device.system_of_record);
    assert!(device.required_columns.contains(&"tenant_id"));
    assert!(device.required_columns.contains(&"organization_id"));
    assert!(device.required_columns.contains(&"owner_type"));
    assert!(device.required_columns.contains(&"owner_id"));
    assert!(device
        .required_indexes
        .contains(&"uk_iot_device_tenant_device_key"));
    assert!(device
        .required_indexes
        .contains(&"idx_iot_device_tenant_product_status"));

    let outbox = table_contract("iot_outbox_event").expect("outbox contract");
    assert_eq!(outbox.profile, TableProfile::OutboxEvent);
    assert!(outbox.required_columns.contains(&"event_type"));
    assert!(outbox.required_columns.contains(&"aggregate_id"));
    assert!(outbox.required_columns.contains(&"event_version"));
    assert!(outbox.required_columns.contains(&"payload_hash"));
}

#[test]
fn catalog_never_declares_appbase_iam_owned_tables() {
    for forbidden in [
        "iam_tenant",
        "iam_organization",
        "iam_user",
        "iam_role",
        "iam_permission",
        "iam_policy",
        "iam_credential",
    ] {
        assert!(
            table_contract(forbidden).is_none(),
            "AIoT must not own {forbidden}"
        );
    }
}

#[test]
fn every_catalog_table_has_a_contract() {
    for table in IOT_TABLES {
        assert!(
            table_contract(table.name).is_some(),
            "{} is listed but has no table contract",
            table.name
        );
    }
}

#[test]
fn protocol_ingest_storage_ports_define_unit_of_work_boundaries() {
    let ports = standard_protocol_ingest_storage_ports();

    assert!(ports.contains(&"DeviceRepository"));
    assert!(ports.contains(&"DeviceSessionRepository"));
    assert!(ports.contains(&"TelemetryRepository"));
    assert!(ports.contains(&"DeviceTwinRepository"));
    assert!(ports.contains(&"CommandDeliveryRepository"));
    assert!(ports.contains(&"FirmwareDeploymentRepository"));
    assert!(ports.contains(&"ProtocolDeadLetterRepository"));
    assert!(ports.contains(&"OutboxEventRepository"));
}

#[test]
fn in_memory_device_repository_supports_scoped_crud_lifecycle() {
    let repo = InMemoryAiotDeviceRepository::new();
    let association = AiotStorageAssociation::tenant_org(100001, 0);
    let create =
        AiotDeviceCreateCommand::new(association.clone(), "device-001", "Front Door", "1001")
            .with_client_id("client-1")
            .with_chip_family("esp32_s3");

    let created = repo.create_device(create).expect("create device");
    assert_eq!(created.id, "1");
    assert_eq!(created.device_id, "device-001");
    assert_eq!(created.display_name, "Front Door");
    assert_eq!(created.product_id, "1001");
    assert_eq!(created.status, "active");

    let listed = repo
        .list_devices(&association, OffsetListPageParams::parse(None, None))
        .expect("list devices");
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].device_id, "device-001");

    let retrieved = repo
        .get_device(&association, "device-001")
        .expect("retrieve device");
    assert_eq!(retrieved.display_name, "Front Door");

    let updated = repo
        .update_device(
            AiotDeviceUpdateCommand::new(association.clone(), "device-001")
                .with_display_name("Front Door Updated")
                .with_status("inactive")
                .with_metadata_json(r#"{"firmware":"1.0.1"}"#),
        )
        .expect("update device");
    assert_eq!(updated.display_name, "Front Door Updated");
    assert_eq!(updated.status, "inactive");
    assert_eq!(
        updated.metadata_json.as_deref(),
        Some(r#"{"firmware":"1.0.1"}"#)
    );

    repo.delete_device(&association, "device-001")
        .expect("delete device");
    assert!(repo.get_device(&association, "device-001").is_none());
}

#[test]
fn in_memory_device_repository_rejects_duplicate_and_cross_scope_operations() {
    let repo = InMemoryAiotDeviceRepository::new();
    let assoc_a = AiotStorageAssociation::tenant_org(100001, 0);
    let assoc_b = AiotStorageAssociation::tenant_org(10002, 20002);

    repo.create_device(AiotDeviceCreateCommand::new(
        assoc_a.clone(),
        "shared-device",
        "A",
        "1001",
    ))
    .expect("create scoped device");

    let duplicate = repo.create_device(AiotDeviceCreateCommand::new(
        assoc_a.clone(),
        "shared-device",
        "A2",
        "1002",
    ));
    assert_eq!(
        duplicate.err(),
        Some(AiotDeviceRepositoryError::DuplicateDeviceId)
    );

    // Same device id in different tenant/org scope is allowed.
    repo.create_device(AiotDeviceCreateCommand::new(
        assoc_b.clone(),
        "shared-device",
        "B",
        "2001",
    ))
    .expect("create cross-scope device");

    assert_eq!(
        repo.list_devices(&assoc_a, OffsetListPageParams::parse(None, None))
            .expect("list a")
            .items
            .len(),
        1
    );
    assert_eq!(
        repo.list_devices(&assoc_b, OffsetListPageParams::parse(None, None))
            .expect("list b")
            .items
            .len(),
        1
    );
    assert_eq!(
        repo.get_device(&assoc_b, "shared-device")
            .unwrap()
            .display_name,
        "B"
    );
    assert_eq!(
        repo.update_device(
            AiotDeviceUpdateCommand::new(assoc_b.clone(), "missing").with_status("inactive")
        )
        .err(),
        Some(AiotDeviceRepositoryError::NotFound)
    );
}

#[test]
fn in_memory_device_repository_rejects_non_numeric_product_id() {
    let repo = InMemoryAiotDeviceRepository::new();
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
fn in_memory_command_repository_supports_create_and_scoped_list() {
    let repo = InMemoryAiotCommandRepository::new();
    let assoc_a = AiotStorageAssociation::tenant_org(100001, 0);
    let assoc_b = AiotStorageAssociation::tenant_org(10002, 20002);

    let created = repo
        .create_command(
            AiotCommandCreateCommand::new(
                assoc_a.clone(),
                "device-001",
                "speaker",
                "play",
            )
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
    assert_eq!(created.command_name, "play");

    repo.create_command(AiotCommandCreateCommand::new(
        assoc_b.clone(),
        "device-001",
        "speaker",
        "play",
    ))
    .expect("create command in other tenant");

    let list_a = repo
        .list_commands(
            &assoc_a,
            "device-001",
            OffsetListPageParams::parse(None, None),
        )
        .expect("list commands in tenant a");
    let list_b = repo
        .list_commands(
            &assoc_b,
            "device-001",
            OffsetListPageParams::parse(None, None),
        )
        .expect("list commands in tenant b");
    assert_eq!(list_a.items.len(), 1);
    assert_eq!(list_b.items.len(), 1);
    assert_eq!(
        list_a.items[0].request_media_resource_id.as_deref(),
        Some("media-res-001")
    );
}

#[test]
fn in_memory_command_repository_supports_cancel_command() {
    let repo = InMemoryAiotCommandRepository::new();
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    let created = repo
        .create_command(
            AiotCommandCreateCommand::new(
                association.clone(),
                "device-cancel-001",
                "speaker",
                "play",
            )
            .with_command_id("cmd-cancel-001"),
        )
        .expect("create command");
    assert_eq!(created.status, "accepted");

    let cancelled = repo
        .cancel_command(&association, "device-cancel-001", "cmd-cancel-001")
        .expect("cancel command")
        .expect("command exists");
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
        .expect("cancel missing");
    assert!(missing.is_none());
}

#[test]
fn in_memory_device_session_repository_supports_disconnect_lifecycle() {
    let repo = InMemoryAiotDeviceSessionRepository::new();
    let association = AiotStorageAssociation::tenant_org(100001, 0);
    let device_id = "device-session-001";
    let session_id = "session-device-session-001-primary";

    assert!(!repo
        .is_session_disconnected(&association, device_id, session_id)
        .expect("query initial session state"));

    assert!(repo
        .disconnect_session(&association, device_id, session_id)
        .expect("disconnect session first time"));
    assert!(repo
        .is_session_disconnected(&association, device_id, session_id)
        .expect("query disconnected session state"));
    assert!(!repo
        .disconnect_session(&association, device_id, session_id)
        .expect("disconnect session second time"));
}

#[test]
fn in_memory_event_repository_supports_record_and_scoped_list() {
    let repo = InMemoryAiotEventRepository::new();
    let assoc = AiotStorageAssociation::tenant_org(100001, 0);

    repo.record_event(
        AiotDeviceEventCreateCommand::new(
            assoc.clone(),
            "device-001",
            "iot.device.media_frame.ingested",
        )
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

    repo.record_event(AiotDeviceEventCreateCommand::new(
        assoc.clone(),
        "device-002",
        "iot.device.media_frame.ingested",
    ))
    .expect("record other device event");

    let all = repo
        .list_events(&assoc, None, OffsetListPageParams::parse(None, None))
        .expect("list all events");
    let scoped = repo
        .list_events(
            &assoc,
            Some("device-001"),
            OffsetListPageParams::parse(None, None),
        )
        .expect("list device events");
    assert_eq!(all.items.len(), 2);
    assert_eq!(scoped.items.len(), 1);
    assert_eq!(scoped.items[0].device_id, "device-001");
    assert_eq!(scoped.items[0].protocol_id, "xiaozhi.websocket");
}

#[test]
fn in_memory_twin_repository_supports_upsert_and_snapshot_read() {
    let repo = InMemoryAiotDeviceTwinRepository::new();
    let assoc = AiotStorageAssociation::tenant_org(100001, 0);

    let empty = repo
        .get_twin_snapshot(&assoc, "device-001")
        .expect("empty snapshot");
    assert!(empty.desired.is_empty());
    assert!(empty.reported.is_empty());

    repo.upsert_twin_property(
        AiotTwinPropertyUpsertCommand::new(assoc.clone(), "device-001", "volume")
            .with_desired_value_json("80")
            .with_reported_value_json("72")
            .with_desired_updated_at("2026-06-01T00:00:01Z")
            .with_reported_updated_at("2026-06-01T00:00:02Z"),
    )
    .expect("upsert twin property");

    let snapshot = repo
        .get_twin_snapshot(&assoc, "device-001")
        .expect("snapshot");
    assert_eq!(
        snapshot.desired.get("volume").map(String::as_str),
        Some("80")
    );
    assert_eq!(
        snapshot.reported.get("volume").map(String::as_str),
        Some("72")
    );
    assert!(snapshot.desired_version >= 1);
    assert!(snapshot.reported_version >= 1);
}

#[test]
fn protocol_runtime_tables_have_payload_and_retry_indexes() {
    let dead_letter =
        table_contract("iot_protocol_message_dead_letter").expect("dead letter contract");
    assert_eq!(dead_letter.profile, TableProfile::EventLog);
    assert_eq!(dead_letter.write_owner, "sdkwork-aiot-service-host");
    assert!(dead_letter.required_columns.contains(&"protocol_id"));
    assert!(dead_letter.required_columns.contains(&"adapter_id"));
    assert!(dead_letter.required_columns.contains(&"payload_ref"));
    assert!(dead_letter.required_columns.contains(&"payload_hash"));
    assert!(dead_letter
        .required_indexes
        .contains(&"idx_iot_protocol_dead_letter_tenant_created"));

    let outbox = table_contract("iot_outbox_event").expect("outbox contract");
    assert_eq!(outbox.write_owner, "sdkwork-iot-device-service");
    assert!(outbox.required_columns.contains(&"next_attempt_at"));
    assert!(outbox.required_columns.contains(&"attempt_count"));
    assert!(outbox.required_columns.contains(&"event_version"));
    assert!(outbox.required_columns.contains(&"payload_hash"));
    assert!(outbox
        .required_indexes
        .contains(&"idx_iot_outbox_event_status_next_attempt"));

    let media = table_contract("iot_media_resource").expect("media resource contract");
    assert_eq!(media.profile, TableProfile::TenantOwnerEntity);
    assert!(media.required_columns.contains(&"media_resource_id"));
    assert!(media.required_columns.contains(&"resource_snapshot"));
    assert!(media
        .required_indexes
        .contains(&"uk_iot_media_resource_tenant_resource_id"));

    let firmware = table_contract("iot_firmware_artifact").expect("firmware artifact contract");
    assert!(firmware.required_columns.contains(&"media_resource_id"));
    assert!(firmware
        .required_columns
        .contains(&"media_resource_snapshot"));
    assert!(firmware
        .required_indexes
        .contains(&"uk_iot_firmware_artifact_tenant_media_resource"));
}

#[test]
fn protocol_ingest_storage_command_declares_atomic_write_plan() {
    let command = AiotProtocolStorageCommand::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "device-001",
        AiotStorageWriteKind::OpenSession,
        "iot_device_session",
    )
    .with_session_id("session-001")
    .with_trace_id("trace-001")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.device.session.started",
        "device_session",
        "session-001",
        "iot.protocol.ingested",
    ));

    assert_eq!(command.protocol_id, "xiaozhi.websocket");
    assert_eq!(command.adapter_id, "xiaozhi");
    assert_eq!(command.device_id, "device-001");
    assert_eq!(command.kind, AiotStorageWriteKind::OpenSession);
    assert_eq!(command.primary_table, "iot_device_session");
    assert_eq!(command.session_id.as_deref(), Some("session-001"));
    assert_eq!(command.trace_id.as_deref(), Some("trace-001"));
    assert!(command.idempotency_key.is_some());
    assert!(command.requires_transaction);
    assert!(command.dead_letter_on_failure);

    let outbox = command.outbox.expect("outbox intent");
    assert_eq!(outbox.event_type, "iot.device.session.started");
    assert_eq!(outbox.aggregate_type, "device_session");
    assert_eq!(outbox.aggregate_id, "session-001");
    assert_eq!(outbox.topic, "iot.protocol.ingested");
    assert_eq!(outbox.event_version, "1");
    assert_eq!(outbox.payload_json, "{}");
    assert!(outbox.payload_hash.is_none());
    assert_eq!(outbox.initial_status, "pending");
}

#[test]
fn protocol_storage_command_can_preserve_message_trace_and_idempotency_metadata() {
    let command = AiotProtocolStorageCommand::new(
        "mqtt.v5",
        "mqtt",
        "device-001",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_message_id("msg-001")
    .with_correlation_id("corr-001")
    .with_trace_id("trace-001")
    .with_idempotency_key("idem-001");

    assert_eq!(command.message_id.as_deref(), Some("msg-001"));
    assert_eq!(command.correlation_id.as_deref(), Some("corr-001"));
    assert_eq!(command.trace_id.as_deref(), Some("trace-001"));
    assert_eq!(command.idempotency_key.as_deref(), Some("idem-001"));
}

#[test]
fn protocol_storage_command_carries_appbase_association_fields_without_iam_dependency() {
    let command = AiotProtocolStorageCommand::new(
        "mqtt.v5",
        "mqtt",
        "device-001",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_association(AiotStorageAssociation::tenant_org(100001, 0).with_data_scope(7));

    assert_eq!(command.association.tenant_id, 100001);
    assert_eq!(command.association.organization_id, 0);
    assert_eq!(command.association.data_scope, 7);
    assert_eq!(command.association.user_id, None);
    assert_eq!(command.association.owner_type, None);
    assert_eq!(command.association.owner_id, None);
}

#[test]
fn protocol_dead_letter_intent_preserves_protocol_context_without_payload_logging() {
    let dead_letter = AiotProtocolDeadLetterIntent::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "decode.unsupported_message_type",
        "object-store://payloads/trace-001",
    )
    .with_device_id("device-001")
    .with_trace_id("trace-001");

    assert_eq!(dead_letter.protocol_id, "xiaozhi.websocket");
    assert_eq!(dead_letter.adapter_id, "xiaozhi");
    assert_eq!(dead_letter.reason_code, "decode.unsupported_message_type");
    assert_eq!(
        dead_letter.payload_ref.as_deref(),
        Some("object-store://payloads/trace-001")
    );
    assert_eq!(dead_letter.device_id.as_deref(), Some("device-001"));
    assert!(dead_letter.raw_payload.is_none());
}

#[test]
fn protocol_dead_letter_intent_carries_appbase_association_fields_without_iam_dependency() {
    let dead_letter = AiotProtocolDeadLetterIntent::new(
        "mqtt.v5",
        "mqtt",
        "decode.invalid_frame",
        "object-store://payloads/msg-009",
    )
    .with_association(AiotStorageAssociation::tenant_org(100001, 0).with_data_scope(7));

    assert_eq!(dead_letter.association.tenant_id, 100001);
    assert_eq!(dead_letter.association.organization_id, 0);
    assert_eq!(dead_letter.association.data_scope, 7);
}

#[test]
fn retry_policy_uses_bounded_backoff_and_dead_letter_threshold() {
    let policy = AiotRetryPolicy::standard();

    assert_eq!(policy.max_attempts, 12);
    assert_eq!(policy.dead_letter_after_attempts, 12);
    assert_eq!(policy.backoff_seconds(0), 1);
    assert_eq!(policy.backoff_seconds(1), 2);
    assert_eq!(policy.backoff_seconds(5), 32);
    assert_eq!(policy.backoff_seconds(99), policy.max_backoff_seconds);
    assert!(!policy.should_dead_letter(11));
    assert!(policy.should_dead_letter(12));
}

#[test]
fn dead_letter_reason_catalog_is_standardized_for_protocol_plugins() {
    let reasons = standard_dead_letter_reason_catalog();

    assert!(reasons.contains(&"decode.unsupported_message_type"));
    assert!(reasons.contains(&"auth.failed"));
    assert!(reasons.contains(&"storage.write_failed"));
    assert!(reasons.contains(&"backpressure.dead_letter_non_critical"));
    assert!(reasons.iter().all(|reason| reason.contains('.')));
}

#[test]
fn storage_command_resolves_standard_repository_port_from_write_kind() {
    for (kind, expected_port, expected_table) in [
        (
            AiotStorageWriteKind::OpenSession,
            "DeviceSessionRepository",
            "iot_device_session",
        ),
        (
            AiotStorageWriteKind::KeepAlive,
            "DeviceOnlineLeaseRepository",
            "iot_device_online_lease",
        ),
        (
            AiotStorageWriteKind::RecordTelemetry,
            "TelemetryRepository",
            "iot_telemetry_event",
        ),
        (
            AiotStorageWriteKind::DispatchCommand,
            "CommandDeliveryRepository",
            "iot_command_delivery",
        ),
        (
            AiotStorageWriteKind::RecordCommandResult,
            "CommandResultRepository",
            "iot_command_result",
        ),
        (
            AiotStorageWriteKind::EvaluateOta,
            "FirmwareDeploymentRepository",
            "iot_firmware_deployment",
        ),
    ] {
        let route = kind.storage_route();

        assert_eq!(route.repository_port, expected_port);
        assert_eq!(route.primary_table, expected_table);
        assert!(route.transactional);
    }
}

#[test]
fn every_storage_write_kind_routes_to_a_declared_protocol_ingest_port() {
    let declared_ports = standard_protocol_ingest_storage_ports();

    for kind in [
        AiotStorageWriteKind::OpenSession,
        AiotStorageWriteKind::Authenticate,
        AiotStorageWriteKind::KeepAlive,
        AiotStorageWriteKind::CloseSession,
        AiotStorageWriteKind::ProvisionDevice,
        AiotStorageWriteKind::RecordTelemetry,
        AiotStorageWriteKind::ApplyDesiredTwin,
        AiotStorageWriteKind::DispatchCommand,
        AiotStorageWriteKind::RecordCommandAck,
        AiotStorageWriteKind::RecordCommandResult,
        AiotStorageWriteKind::ProcessMediaFrame,
        AiotStorageWriteKind::EvaluateOta,
        AiotStorageWriteKind::DispatchOta,
        AiotStorageWriteKind::UpdateGatewayTopology,
        AiotStorageWriteKind::RecordSecurityEvent,
        AiotStorageWriteKind::RecordDiagnostic,
    ] {
        let route = kind.storage_route();

        assert!(
            declared_ports.contains(&route.repository_port),
            "{} routes to undeclared port {}",
            kind.as_str(),
            route.repository_port
        );
        assert!(
            table_contract(route.primary_table).is_some(),
            "{} routes to undeclared table {}",
            kind.as_str(),
            route.primary_table
        );
    }
}

#[test]
fn protocol_ingest_unit_of_work_port_accepts_command_and_dead_letter_intents() {
    struct RecordingUnitOfWork;

    impl AiotProtocolIngestUnitOfWork for RecordingUnitOfWork {
        fn execute_protocol_command(
            &self,
            command: &AiotProtocolStorageCommand,
        ) -> AiotStorageWriteReceipt {
            AiotStorageWriteReceipt::accepted(
                command.kind,
                command.primary_table,
                command
                    .outbox
                    .as_ref()
                    .map(|outbox| outbox.event_type.clone()),
            )
        }

        fn record_dead_letter(
            &self,
            intent: &AiotProtocolDeadLetterIntent,
        ) -> AiotStorageWriteReceipt {
            AiotStorageWriteReceipt::dead_lettered(intent.reason_code.clone())
        }
    }

    let uow = RecordingUnitOfWork;
    let command = AiotProtocolStorageCommand::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "device-001",
        AiotStorageWriteKind::RecordTelemetry,
        "iot_telemetry_event",
    )
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.telemetry.received",
        "device",
        "device-001",
        "iot.protocol.ingested",
    ));
    let receipt = uow.execute_protocol_command(&command);

    assert!(receipt.accepted);
    assert_eq!(
        receipt.write_kind,
        Some(AiotStorageWriteKind::RecordTelemetry)
    );
    assert_eq!(
        receipt.primary_table.as_deref(),
        Some("iot_telemetry_event")
    );
    assert_eq!(
        receipt.outbox_event_type.as_deref(),
        Some("iot.telemetry.received")
    );

    let dead_letter = AiotProtocolDeadLetterIntent::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "storage.write_failed",
        "object-store://payloads/trace-001",
    );
    let dead_letter_receipt = uow.record_dead_letter(&dead_letter);

    assert!(!dead_letter_receipt.accepted);
    assert_eq!(
        dead_letter_receipt.dead_letter_reason.as_deref(),
        Some("storage.write_failed")
    );
}

#[test]
fn retryable_storage_failure_has_deterministic_retry_or_dead_letter_disposition() {
    let policy = AiotRetryPolicy::standard();
    let retryable = AiotStorageFailure::retryable("storage.write_failed", 3);
    let disposition = retryable.disposition(&policy);

    assert_eq!(
        disposition,
        AiotStorageFailureDisposition::Retry {
            next_attempt_in_seconds: 8
        }
    );

    let exhausted = AiotStorageFailure::retryable("storage.write_failed", 12);
    assert_eq!(
        exhausted.disposition(&policy),
        AiotStorageFailureDisposition::DeadLetter {
            reason_code: "storage.write_failed".to_string()
        }
    );

    let fatal = AiotStorageFailure::fatal("decode.unsupported_message_type");
    assert_eq!(
        fatal.disposition(&policy),
        AiotStorageFailureDisposition::DeadLetter {
            reason_code: "decode.unsupported_message_type".to_string()
        }
    );
}

#[test]
fn in_memory_protocol_uow_records_primary_write_and_outbox_atomically() {
    let uow = InMemoryProtocolIngestUnitOfWork::new();
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
    .with_media_reference(
        "media-res-001",
        Some("obj-blob-001".to_string()),
        Some(r#"{"id":"media-res-001","kind":"audio","source":"object_storage"}"#.to_string()),
    )
    .with_idempotency_key("idem-001")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.device.session.started",
        "device_session",
        "session-001",
        "iot.protocol.ingested",
    ));

    let receipt = uow.execute_protocol_command(&command);
    let snapshot = uow.snapshot();

    assert!(receipt.accepted);
    assert!(!receipt.duplicate);
    assert_eq!(snapshot.primary_writes.len(), 1);
    assert_eq!(snapshot.outbox_events.len(), 1);
    assert_eq!(snapshot.dead_letters.len(), 0);
    assert_eq!(
        snapshot.primary_writes[0].primary_table,
        "iot_device_session"
    );
    assert_eq!(snapshot.primary_writes[0].idempotency_key, "idem-001");
    assert_eq!(
        snapshot.primary_writes[0].message_id.as_deref(),
        Some("msg-001")
    );
    assert_eq!(
        snapshot.primary_writes[0].media_resource_id.as_deref(),
        Some("media-res-001")
    );
    assert_eq!(
        snapshot.primary_writes[0].object_blob_id.as_deref(),
        Some("obj-blob-001")
    );
    assert!(snapshot.primary_writes[0]
        .media_resource_snapshot
        .as_deref()
        .unwrap_or_default()
        .contains(r#""kind":"audio""#));
    assert_eq!(
        snapshot.outbox_events[0].event_type,
        "iot.device.session.started"
    );
    assert_eq!(snapshot.outbox_events[0].event_version, "1");
    assert_eq!(snapshot.outbox_events[0].payload_json, "{}");
    assert_eq!(snapshot.outbox_events[0].aggregate_id, "session-001");
}

#[test]
fn in_memory_protocol_uow_preserves_appbase_association_in_primary_and_outbox_records() {
    let uow = InMemoryProtocolIngestUnitOfWork::new();
    let association = AiotStorageAssociation::tenant_org(100001, 0)
        .with_user_id(30001)
        .with_data_scope(7);
    let command = AiotProtocolStorageCommand::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "device-001",
        AiotStorageWriteKind::OpenSession,
        "iot_device_session",
    )
    .with_association(association.clone())
    .with_session_id("session-001")
    .with_idempotency_key("idem-001")
    .with_outbox(AiotOutboxWriteIntent::new(
        "iot.device.session.started",
        "device_session",
        "session-001",
        "iot.protocol.ingested",
    ));

    let receipt = uow.execute_protocol_command(&command);
    let snapshot = uow.snapshot();

    assert!(receipt.accepted);
    assert_eq!(snapshot.primary_writes[0].association, association);
    assert_eq!(snapshot.outbox_events[0].association, association);
}

#[test]
fn in_memory_protocol_uow_enforces_idempotent_ingest_without_duplicate_writes() {
    let uow = InMemoryProtocolIngestUnitOfWork::new();
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
    let snapshot = uow.snapshot();

    assert!(first.accepted);
    assert!(second.accepted);
    assert!(second.duplicate);
    assert_eq!(snapshot.primary_writes.len(), 1);
    assert_eq!(snapshot.outbox_events.len(), 1);
}

#[test]
fn in_memory_protocol_uow_records_dead_letter_without_raw_payload() {
    let uow = InMemoryProtocolIngestUnitOfWork::new();
    let intent = AiotProtocolDeadLetterIntent::new(
        "xiaozhi.websocket",
        "xiaozhi",
        "decode.unsupported_message_type",
        "object-store://payloads/msg-001",
    )
    .with_device_id("device-001")
    .with_trace_id("trace-001");

    let receipt = uow.record_dead_letter(&intent);
    let snapshot = uow.snapshot();

    assert!(!receipt.accepted);
    assert_eq!(
        receipt.dead_letter_reason.as_deref(),
        Some("decode.unsupported_message_type")
    );
    assert_eq!(snapshot.dead_letters.len(), 1);
    assert_eq!(
        snapshot.dead_letters[0].payload_ref.as_deref(),
        Some("object-store://payloads/msg-001")
    );
    assert!(snapshot.dead_letters[0].raw_payload.is_none());
}
