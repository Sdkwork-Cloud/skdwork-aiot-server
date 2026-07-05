//! Resolves rollout target devices and materializes per-device deployment records.

use sdkwork_aiot_storage::{AiotDeviceRepository, AiotStorageAssociation};
use serde_json::Value;

pub const ENTITY_FIRMWARE_DEPLOYMENT: &str = "firmware_deployment";
const DEPLOYMENT_STATE_PENDING: &str = "pending";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RolloutTargetPolicyError {
    InvalidJson,
}

pub fn resolve_rollout_target_device_ids(
    target_policy_json: &str,
    association: &AiotStorageAssociation,
    device_repository: &dyn AiotDeviceRepository,
) -> Result<Vec<String>, RolloutTargetPolicyError> {
    let policy = serde_json::from_str::<Value>(target_policy_json)
        .map_err(|_| RolloutTargetPolicyError::InvalidJson)?;

    let scope = policy.get("scope").and_then(Value::as_str).unwrap_or("all");
    let batch = policy
        .get("batch")
        .and_then(Value::as_u64)
        .map(|value| value as i64);

    let mut candidates = match scope {
        "devices" => policy
            .get("deviceIds")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        "product" => {
            let product_id = policy
                .get("productId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            device_repository
                .list_device_ids_for_rollout(association, Some(product_id), batch)
                .map_err(|_| RolloutTargetPolicyError::InvalidJson)?
        }
        _ => device_repository
            .list_device_ids_for_rollout(association, None, batch)
            .map_err(|_| RolloutTargetPolicyError::InvalidJson)?,
    };

    candidates.sort();
    candidates.dedup();

    if scope == "devices" {
        if let Some(limit) = batch {
            let max = limit.max(0) as usize;
            candidates.truncate(max);
        }
    }

    Ok(candidates)
}

pub fn rollout_force_from_policy(target_policy_json: &str) -> u32 {
    serde_json::from_str::<Value>(target_policy_json)
        .ok()
        .and_then(|policy| policy.get("force").and_then(Value::as_u64))
        .map(|value| value as u32)
        .unwrap_or(0)
}

pub fn firmware_deployment_payload_json(
    deployment_id: &str,
    association: &AiotStorageAssociation,
    rollout_id: &str,
    artifact_id: &str,
    device_id: &str,
    force: u32,
) -> String {
    format!(
        r#"{{"deploymentId":"{deployment_id}","tenantId":{},"organizationId":{},"rolloutId":"{rollout_id}","artifactId":"{artifact_id}","deviceId":"{device_id}","deploymentState":"{DEPLOYMENT_STATE_PENDING}","force":{force}}}"#,
        association.tenant_id, association.organization_id
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_aiot_storage::{
        AiotDeviceRecord, AiotDeviceRepository, AiotDeviceRepositoryError, AiotOffsetListResult,
        OffsetListPageParams,
    };

    struct StubDeviceRepository {
        devices: Vec<AiotDeviceRecord>,
    }

    impl AiotDeviceRepository for StubDeviceRepository {
        fn create_device(
            &self,
            _command: sdkwork_aiot_storage::AiotDeviceCreateCommand,
        ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError> {
            Err(AiotDeviceRepositoryError::PersistenceFailure)
        }

        fn get_device(
            &self,
            _association: &AiotStorageAssociation,
            _device_id: &str,
        ) -> Option<AiotDeviceRecord> {
            None
        }

        fn list_devices(
            &self,
            _association: &AiotStorageAssociation,
            _params: OffsetListPageParams,
        ) -> Result<AiotOffsetListResult<AiotDeviceRecord>, AiotDeviceRepositoryError> {
            Ok(AiotOffsetListResult::empty())
        }

        fn list_device_ids_for_rollout(
            &self,
            _association: &AiotStorageAssociation,
            product_id: Option<&str>,
            limit: Option<i64>,
        ) -> Result<Vec<String>, AiotDeviceRepositoryError> {
            let mut ids = self
                .devices
                .iter()
                .filter(|device| product_id.is_none_or(|product| device.product_id == product))
                .map(|device| device.device_id.clone())
                .collect::<Vec<_>>();
            ids.sort();
            ids.dedup();
            if let Some(limit) = limit {
                ids.truncate(limit.max(0) as usize);
            }
            Ok(ids)
        }

        fn update_device(
            &self,
            _command: sdkwork_aiot_storage::AiotDeviceUpdateCommand,
        ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError> {
            Err(AiotDeviceRepositoryError::PersistenceFailure)
        }

        fn delete_device(
            &self,
            _association: &AiotStorageAssociation,
            _device_id: &str,
        ) -> Result<(), AiotDeviceRepositoryError> {
            Err(AiotDeviceRepositoryError::PersistenceFailure)
        }
    }

    fn sample_device(device_id: &str, product_id: &str) -> AiotDeviceRecord {
        AiotDeviceRecord {
            id: "1".to_string(),
            tenant_id: 100001,
            organization_id: 0,
            device_id: device_id.to_string(),
            display_name: device_id.to_string(),
            product_id: product_id.to_string(),
            client_id: None,
            chip_family: None,
            status: "active".to_string(),
            metadata_json: None,
            last_seen_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn association() -> AiotStorageAssociation {
        AiotStorageAssociation::tenant_org(100001, 0)
    }

    #[test]
    fn resolve_all_scope_honors_batch_limit() {
        let repo = StubDeviceRepository {
            devices: vec![
                sample_device("device-b", "1009"),
                sample_device("device-a", "1009"),
                sample_device("device-c", "1009"),
            ],
        };
        let targets = resolve_rollout_target_device_ids(
            r#"{"scope":"all","batch":2}"#,
            &association(),
            &repo,
        )
        .expect("targets");
        assert_eq!(
            targets,
            vec!["device-a".to_string(), "device-b".to_string()]
        );
    }

    #[test]
    fn resolve_devices_scope_uses_explicit_ids() {
        let repo = StubDeviceRepository {
            devices: vec![sample_device("device-a", "1009")],
        };
        let targets = resolve_rollout_target_device_ids(
            r#"{"scope":"devices","deviceIds":["device-x","device-a"]}"#,
            &association(),
            &repo,
        )
        .expect("targets");
        assert_eq!(
            targets,
            vec!["device-a".to_string(), "device-x".to_string()]
        );
    }

    #[test]
    fn invalid_policy_json_returns_error() {
        let repo = StubDeviceRepository { devices: vec![] };
        let error = resolve_rollout_target_device_ids("{", &association(), &repo)
            .expect_err("invalid json");
        assert_eq!(error, RolloutTargetPolicyError::InvalidJson);
    }
}
