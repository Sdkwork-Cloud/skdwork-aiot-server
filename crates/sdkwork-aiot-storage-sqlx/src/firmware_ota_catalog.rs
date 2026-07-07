//! Rollout-aware firmware OTA resolution for edge gateway device polls.

use std::sync::Arc;

use serde_json::Value;
use sqlx::Row;

use crate::blocking_device_pool::DeviceDatabaseEngine;
use crate::device_database::open_aiot_device_database_from_env;
use crate::dialect_sql::adapt_sqlite_placeholders;
use crate::persisted_entity::{SqlitePersistedEntityError, SqlitePersistedEntityRepository};

pub const ENTITY_FIRMWARE_DEPLOYMENT: &str = "firmware_deployment";
pub const ENTITY_FIRMWARE_ARTIFACT: &str = "firmware_artifact";
pub const MAX_OTA_DEPLOYMENT_SCAN: i64 = 200;
pub const DEFAULT_ROLLOUT_DEVICE_BATCH: i64 = 200;

const DEPLOYMENT_STATE_PENDING: &str = "pending";
const DEPLOYMENT_STATE_OFFERED: &str = "offered";
const DEPLOYMENT_STATE_COMPLETED: &str = "completed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareOtaHint {
    pub version: String,
    pub url: String,
    pub force: u32,
}

#[derive(Debug, Clone)]
struct ScopedEntityRow {
    payload_json: String,
}

#[derive(Clone)]
pub struct FirmwareOtaCatalog {
    store: Arc<SqlitePersistedEntityRepository>,
}

impl FirmwareOtaCatalog {
    pub fn from_repository(store: SqlitePersistedEntityRepository) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    pub fn open_from_env() -> Option<Self> {
        open_aiot_device_database_from_env()
            .ok()
            .and_then(|database| database.persisted_entity_repository().ok())
            .map(Self::from_repository)
    }

    pub fn resolve_for_device(&self, device_id: &str) -> Option<FirmwareOtaHint> {
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return None;
        }

        let deployment = self
            .list_deployments_for_device(device_id, DEPLOYMENT_STATE_PENDING)
            .into_iter()
            .max_by(|left, right| left.deployment_id.cmp(&right.deployment_id))?;

        let artifact = self
            .get_entity(
                deployment.tenant_id,
                deployment.organization_id,
                ENTITY_FIRMWARE_ARTIFACT,
                &deployment.artifact_id,
            )
            .and_then(|row| parse_artifact_row(&row))?;

        let (version, url, force) = resolve_firmware_download_url(
            &artifact.artifact_id,
            &artifact.version,
            &artifact.resource_json,
        )?;

        let _ = self.mark_deployment_offered(
            deployment.tenant_id,
            deployment.organization_id,
            &deployment.deployment_id,
        );

        Some(FirmwareOtaHint {
            version,
            url,
            force: deployment.force.max(force),
        })
    }

    /// Marks a deployment completed after the device reports a successful OTA apply.
    pub fn mark_deployment_completed(
        &self,
        tenant_id: i64,
        organization_id: i64,
        deployment_id: &str,
    ) -> Result<(), SqlitePersistedEntityError> {
        self.set_deployment_state(
            tenant_id,
            organization_id,
            deployment_id,
            DEPLOYMENT_STATE_COMPLETED,
        )
    }

    /// Marks the latest offered deployment for a device completed after OTA apply ingest.
    pub fn mark_offered_deployment_completed_for_device(
        &self,
        device_id: &str,
    ) -> Result<(), SqlitePersistedEntityError> {
        let device_id = device_id.trim();
        if device_id.is_empty() {
            return Err(SqlitePersistedEntityError::NotFound);
        }

        let deployment = self
            .list_deployments_for_device(device_id, DEPLOYMENT_STATE_OFFERED)
            .into_iter()
            .max_by(|left, right| left.deployment_id.cmp(&right.deployment_id))
            .ok_or(SqlitePersistedEntityError::NotFound)?;

        self.mark_deployment_completed(
            deployment.tenant_id,
            deployment.organization_id,
            &deployment.deployment_id,
        )
    }

    fn set_deployment_state(
        &self,
        tenant_id: i64,
        organization_id: i64,
        deployment_id: &str,
        deployment_state: &str,
    ) -> Result<(), SqlitePersistedEntityError> {
        use sdkwork_aiot_storage::AiotStorageAssociation;
        let association = AiotStorageAssociation::tenant_org(tenant_id, organization_id);
        let Some(entity) =
            self.store
                .get_entity(&association, ENTITY_FIRMWARE_DEPLOYMENT, deployment_id)
        else {
            return Err(SqlitePersistedEntityError::NotFound);
        };
        let mut value: Value = serde_json::from_str(&entity.payload_json)
            .map_err(|_| SqlitePersistedEntityError::PersistenceFailure)?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "deploymentState".to_string(),
                Value::String(deployment_state.to_string()),
            );
        }
        let payload_json = serde_json::to_string(&value)
            .map_err(|_| SqlitePersistedEntityError::PersistenceFailure)?;
        self.store.upsert_entity(
            &association,
            ENTITY_FIRMWARE_DEPLOYMENT,
            deployment_id,
            &payload_json,
        )
    }

    fn get_entity(
        &self,
        tenant_id: i64,
        organization_id: i64,
        entity_kind: &str,
        entity_key: &str,
    ) -> Option<ScopedEntityRow> {
        use sdkwork_aiot_storage::AiotStorageAssociation;
        let association = AiotStorageAssociation::tenant_org(tenant_id, organization_id);
        self.store
            .get_entity(&association, entity_kind, entity_key)
            .map(|entity| ScopedEntityRow {
                payload_json: entity.payload_json,
            })
    }

    fn mark_deployment_offered(
        &self,
        tenant_id: i64,
        organization_id: i64,
        deployment_id: &str,
    ) -> Result<(), SqlitePersistedEntityError> {
        self.set_deployment_state(
            tenant_id,
            organization_id,
            deployment_id,
            DEPLOYMENT_STATE_OFFERED,
        )
    }

    fn list_deployments_for_device(
        &self,
        device_id: &str,
        deployment_state: &str,
    ) -> Vec<ParsedDeployment> {
        self.store
            .blocking_pool()
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let device_scope_sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT tenant_id, organization_id
                     FROM iot_device
                     WHERE device_id = ?1
                     LIMIT 1",
                );
                let device_id = device_id.to_string();
                let deployment_state = deployment_state.to_string();
                let device_scope: Option<(i64, i64)> = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let row = sqlx::query(&device_scope_sql)
                            .bind(&device_id)
                            .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        row.map(|row| {
                            (
                                row.try_get::<i64, _>("tenant_id").expect("tenant_id"),
                                row.try_get::<i64, _>("organization_id")
                                    .expect("organization_id"),
                            )
                        })
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let row = sqlx::query(&device_scope_sql)
                            .bind(&device_id)
                            .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        row.map(|row| {
                            (
                                row.try_get::<i64, _>("tenant_id").expect("tenant_id"),
                                row.try_get::<i64, _>("organization_id")
                                    .expect("organization_id"),
                            )
                        })
                    }
                };

                let entity_kind = ENTITY_FIRMWARE_DEPLOYMENT.to_string();
                let limit = MAX_OTA_DEPLOYMENT_SCAN;

                fn map_deployment_rows(
                    rows: Vec<sqlx::sqlite::SqliteRow>,
                ) -> Vec<ParsedDeployment> {
                    rows.into_iter()
                        .filter_map(|row| {
                            Some(ScopedEntityRow {
                                payload_json: row.try_get("payload_json").ok()?,
                            })
                        })
                        .filter_map(|row| parse_deployment_row(&row))
                        .collect()
                }

                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        if let Some((tenant_id, organization_id)) = device_scope {
                            let list_sql = adapt_sqlite_placeholders(
                                dialect,
                                "SELECT payload_json
                                 FROM iot_admin_entity
                                 WHERE tenant_id = ?1
                                   AND organization_id = ?2
                                   AND entity_kind = ?3
                                   AND status = 1
                                   AND json_extract(payload_json, '$.deviceId') = ?4
                                   AND json_extract(payload_json, '$.deploymentState') = ?5
                                 ORDER BY entity_key DESC
                                 LIMIT ?6",
                            );
                            let rows = sqlx::query(&list_sql)
                                .bind(tenant_id)
                                .bind(organization_id)
                                .bind(&entity_kind)
                                .bind(&device_id)
                                .bind(&deployment_state)
                                .bind(limit)
                                .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                                .await?;
                            Ok::<Vec<ParsedDeployment>, sqlx::Error>(map_deployment_rows(rows))
                        } else {
                            let list_sql = adapt_sqlite_placeholders(
                                dialect,
                                "SELECT payload_json
                                 FROM iot_admin_entity
                                 WHERE entity_kind = ?1
                                   AND status = 1
                                   AND json_extract(payload_json, '$.deviceId') = ?2
                                   AND json_extract(payload_json, '$.deploymentState') = ?3
                                 ORDER BY entity_key DESC
                                 LIMIT ?4",
                            );
                            let rows = sqlx::query(&list_sql)
                                .bind(&entity_kind)
                                .bind(&device_id)
                                .bind(&deployment_state)
                                .bind(limit)
                                .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                                .await?;
                            Ok::<Vec<ParsedDeployment>, sqlx::Error>(map_deployment_rows(rows))
                        }
                    }
                    DeviceDatabaseEngine::Postgres => {
                        if let Some((tenant_id, organization_id)) = device_scope {
                            let list_sql = adapt_sqlite_placeholders(
                                dialect,
                                "SELECT payload_json
                                 FROM iot_admin_entity
                                 WHERE tenant_id = ?1
                                   AND organization_id = ?2
                                   AND entity_kind = ?3
                                   AND status = 1
                                   AND payload_json::jsonb ->> 'deviceId' = ?4
                                   AND payload_json::jsonb ->> 'deploymentState' = ?5
                                 ORDER BY entity_key DESC
                                 LIMIT ?6",
                            );
                            let rows = sqlx::query(&list_sql)
                                .bind(tenant_id)
                                .bind(organization_id)
                                .bind(&entity_kind)
                                .bind(&device_id)
                                .bind(&deployment_state)
                                .bind(limit)
                                .fetch_all(pool.postgres_pool().expect("postgres pool"))
                                .await?;
                            Ok::<Vec<ParsedDeployment>, sqlx::Error>(
                                rows.into_iter()
                                    .filter_map(|row| {
                                        Some(ScopedEntityRow {
                                            payload_json: row.try_get("payload_json").ok()?,
                                        })
                                    })
                                    .filter_map(|row| parse_deployment_row(&row))
                                    .collect(),
                            )
                        } else {
                            let list_sql = adapt_sqlite_placeholders(
                                dialect,
                                "SELECT payload_json
                                 FROM iot_admin_entity
                                 WHERE entity_kind = ?1
                                   AND status = 1
                                   AND payload_json::jsonb ->> 'deviceId' = ?2
                                   AND payload_json::jsonb ->> 'deploymentState' = ?3
                                 ORDER BY entity_key DESC
                                 LIMIT ?4",
                            );
                            let rows = sqlx::query(&list_sql)
                                .bind(&entity_kind)
                                .bind(&device_id)
                                .bind(&deployment_state)
                                .bind(limit)
                                .fetch_all(pool.postgres_pool().expect("postgres pool"))
                                .await?;
                            Ok::<Vec<ParsedDeployment>, sqlx::Error>(
                                rows.into_iter()
                                    .filter_map(|row| {
                                        Some(ScopedEntityRow {
                                            payload_json: row.try_get("payload_json").ok()?,
                                        })
                                    })
                                    .filter_map(|row| parse_deployment_row(&row))
                                    .collect(),
                            )
                        }
                    }
                }
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
struct ParsedDeployment {
    deployment_id: String,
    tenant_id: i64,
    organization_id: i64,
    artifact_id: String,
    force: u32,
}

#[derive(Debug, Clone)]
struct ParsedArtifact {
    artifact_id: String,
    version: String,
    resource_json: String,
}

fn parse_deployment_row(row: &ScopedEntityRow) -> Option<ParsedDeployment> {
    let value: Value = serde_json::from_str(&row.payload_json).ok()?;
    Some(ParsedDeployment {
        deployment_id: value.get("deploymentId")?.as_str()?.to_string(),
        tenant_id: value.get("tenantId")?.as_i64()?,
        organization_id: value.get("organizationId")?.as_i64()?,
        artifact_id: value.get("artifactId")?.as_str()?.to_string(),
        force: value
            .get("force")
            .and_then(Value::as_u64)
            .map(|value| value as u32)
            .unwrap_or(0),
    })
}

fn parse_artifact_row(row: &ScopedEntityRow) -> Option<ParsedArtifact> {
    let value: Value = serde_json::from_str(&row.payload_json).ok()?;
    Some(ParsedArtifact {
        artifact_id: value.get("artifactId")?.as_str()?.to_string(),
        version: value.get("version")?.as_str()?.to_string(),
        resource_json: value
            .get("resourceJson")
            .and_then(Value::as_str)
            .unwrap_or("{}")
            .to_string(),
    })
}

pub fn resolve_firmware_download_url(
    artifact_id: &str,
    version: &str,
    resource_json: &str,
) -> Option<(String, String, u32)> {
    if let Ok(value) = serde_json::from_str::<Value>(resource_json) {
        let force = value
            .get("force")
            .and_then(Value::as_u64)
            .map(|value| value as u32)
            .unwrap_or(0);

        for key in ["downloadUrl", "url", "publicUrl"] {
            if let Some(url) = value
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|url| !url.is_empty())
            {
                return Some((version.to_string(), url.to_string(), force));
            }
        }

        if let Some(node_id) = drive_node_id_from_media_resource(&value) {
            if let Some(base) = firmware_download_base_url() {
                return Some((
                    version.to_string(),
                    format!("{base}/drive/nodes/{node_id}"),
                    force,
                ));
            }
        }

        if let Some(blob_id) = value
            .get("objectBlobId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Some(base) = firmware_download_base_url() {
                return Some((
                    version.to_string(),
                    format!("{base}/blobs/{blob_id}"),
                    force,
                ));
            }
        }
    }

    firmware_download_base_url().map(|base| {
        (
            version.to_string(),
            format!("{base}/artifacts/{artifact_id}"),
            0,
        )
    })
}

fn drive_node_id_from_media_resource(value: &Value) -> Option<String> {
    let source = value.get("source").and_then(Value::as_str);
    if source == Some("drive") {
        if let Some(id) = value
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            return Some(id.to_string());
        }
    }

    let uri = value
        .get("uri")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|uri| !uri.is_empty())?;
    let prefix = "drive://nodes/";
    if !uri.starts_with(prefix) {
        return None;
    }
    let node_id = uri[prefix.len()..].trim_matches('/');
    if node_id.is_empty() {
        return None;
    }
    Some(node_id.to_string())
}

fn firmware_download_base_url() -> Option<String> {
    std::env::var("SDKWORK_AIOT_FIRMWARE_DOWNLOAD_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_aiot_storage::AiotStorageAssociation;

    #[test]
    fn resolve_firmware_download_url_prefers_explicit_download_url() {
        let (version, url, force) = resolve_firmware_download_url(
            "artifact-1",
            "2.0.0",
            r#"{"downloadUrl":"https://cdn.example.com/fw.bin","force":1}"#,
        )
        .expect("url");
        assert_eq!(version, "2.0.0");
        assert_eq!(url, "https://cdn.example.com/fw.bin");
        assert_eq!(force, 1);
    }

    #[test]
    fn resolve_firmware_download_url_maps_drive_media_resource_to_gateway_path() {
        std::env::set_var(
            "SDKWORK_AIOT_FIRMWARE_DOWNLOAD_BASE_URL",
            "https://edge.aiot.example.com/iot/firmware",
        );
        let (version, url, force) = resolve_firmware_download_url(
            "artifact-drive",
            "3.0.0",
            r#"{"id":"drive-node-001","kind":"archive","source":"drive","uri":"drive://nodes/drive-node-001"}"#,
        )
        .expect("drive url");
        std::env::remove_var("SDKWORK_AIOT_FIRMWARE_DOWNLOAD_BASE_URL");
        assert_eq!(version, "3.0.0");
        assert_eq!(
            url,
            "https://edge.aiot.example.com/iot/firmware/drive/nodes/drive-node-001"
        );
        assert_eq!(force, 0);
    }

    #[test]
    fn catalog_resolves_pending_deployment_for_device() {
        let store = SqlitePersistedEntityRepository::new_in_memory().expect("repo");
        let catalog = FirmwareOtaCatalog::from_repository(
            SqlitePersistedEntityRepository::from_blocking_pool(store.blocking_pool())
                .expect("repo clone"),
        );
        let association = AiotStorageAssociation::tenant_org(100001, 0);

        let artifact_payload = r#"{"artifactId":"firmware-artifact-0001","version":"2.0.0","resourceJson":"{\"downloadUrl\":\"https://cdn.example.com/fw-2.bin\"}"}"#;
        store
            .upsert_entity(
                &association,
                ENTITY_FIRMWARE_ARTIFACT,
                "firmware-artifact-0001",
                artifact_payload,
            )
            .expect("artifact");

        let deployment_payload = r#"{"deploymentId":"firmware-deployment-0001","tenantId":100001,"organizationId":0,"rolloutId":"firmware-rollout-0001","artifactId":"firmware-artifact-0001","deviceId":"device-ota-001","deploymentState":"pending","force":1}"#;
        store
            .upsert_entity(
                &association,
                ENTITY_FIRMWARE_DEPLOYMENT,
                "firmware-deployment-0001",
                deployment_payload,
            )
            .expect("deployment");

        let hint = catalog
            .resolve_for_device("device-ota-001")
            .expect("ota hint");
        assert_eq!(hint.version, "2.0.0");
        assert_eq!(hint.url, "https://cdn.example.com/fw-2.bin");
        assert_eq!(hint.force, 1);

        let offered = store
            .get_entity(
                &association,
                ENTITY_FIRMWARE_DEPLOYMENT,
                "firmware-deployment-0001",
            )
            .expect("deployment entity");
        assert!(offered
            .payload_json
            .contains(r#""deploymentState":"offered""#));

        assert!(
            catalog.resolve_for_device("device-ota-001").is_none(),
            "offered deployments must not be re-served on subsequent OTA polls",
        );

        catalog
            .mark_offered_deployment_completed_for_device("device-ota-001")
            .expect("complete offered deployment");
        assert!(
            catalog
                .mark_offered_deployment_completed_for_device("device-ota-001")
                .is_err(),
            "completed deployments must not be completed twice",
        );
    }
}
