-- Admin/catalog entity store for firmware artifacts, rollouts, and deployments.

CREATE TABLE IF NOT EXISTS iot_admin_entity (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    entity_kind VARCHAR(64) NOT NULL,
    entity_key VARCHAR(128) NOT NULL,
    payload_json TEXT NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_admin_entity_uuid UNIQUE (uuid),
    CONSTRAINT uk_iot_admin_entity_scope_key UNIQUE (tenant_id, organization_id, entity_kind, entity_key)
);

CREATE INDEX IF NOT EXISTS idx_iot_admin_entity_tenant_kind
    ON iot_admin_entity (tenant_id, entity_kind, status);
