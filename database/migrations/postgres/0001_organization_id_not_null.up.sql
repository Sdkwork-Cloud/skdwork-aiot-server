-- sdkwork:migration
-- id: 0001_organization_id_not_null
-- engine: postgres
-- module: sdkwork-aiot
-- purpose: Enforce organization_id NOT NULL DEFAULT on all tables in the
--   consolidated baseline. NULL rows (pre-standard data anomalies) are
--   backfilled with the platform sentinel before NOT NULL is set, and
--   NOT NULL columns without an explicit default receive the sentinel
--   default, keeping existing deployments consistent with fresh baseline
--   installs.
-- reversible: false
-- rollback: forward-fix (sentinel backfill is the canonical fix; NULL
--   organization rows are data anomalies)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

UPDATE iot_product SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_product ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_product ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_hardware_profile SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_hardware_profile ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_hardware_profile ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_protocol_profile SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_protocol_profile ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_protocol_profile ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_capability_model SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_capability_model ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_capability_model ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_capability_definition SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_capability_definition ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_capability_definition ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_credential SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_credential ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_credential ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_binding SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_binding ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_binding ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_gateway_child_device SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_gateway_child_device ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_gateway_child_device ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_connection SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_connection ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_connection ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_session SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_session ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_session ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_online_lease SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_online_lease ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_online_lease ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_command SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_command ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_command ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_command_delivery SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_command_delivery ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_command_delivery ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_command_result SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_command_result ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_command_result ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_twin SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_twin ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_twin ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_twin_property SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_twin_property ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_twin_property ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_telemetry_latest SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_telemetry_latest ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_telemetry_latest ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_telemetry_event SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_telemetry_event ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_telemetry_event ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_event SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_event ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_event ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_security_event SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_security_event ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_security_event ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_media_resource SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_media_resource ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_media_resource ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_device_media SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_device_media ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_device_media ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_firmware_artifact SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_firmware_artifact ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_firmware_artifact ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_firmware_rollout SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_firmware_rollout ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_firmware_rollout ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_firmware_rollout_target SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_firmware_rollout_target ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_firmware_rollout_target ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_firmware_deployment SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_firmware_deployment ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_firmware_deployment ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_provisioning_challenge SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_provisioning_challenge ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_provisioning_challenge ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_activation_record SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_activation_record ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_activation_record ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_protocol_message_dead_letter SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_protocol_message_dead_letter ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_protocol_message_dead_letter ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_outbox_event SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_outbox_event ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_outbox_event ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_inbox_event SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_inbox_event ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_inbox_event ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_audit_log SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_audit_log ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_audit_log ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_protocol_ingest_record SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_protocol_ingest_record ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_protocol_ingest_record ALTER COLUMN organization_id SET NOT NULL;

UPDATE iot_admin_entity SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE iot_admin_entity ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE iot_admin_entity ALTER COLUMN organization_id SET NOT NULL;

COMMIT;
