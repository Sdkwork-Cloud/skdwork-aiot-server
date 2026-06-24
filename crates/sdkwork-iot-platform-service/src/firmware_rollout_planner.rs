//! Resolves rollout target devices and materializes per-device deployment records.

use sdkwork_aiot_storage::{AiotDeviceRecord, AiotStorageAssociation};
use serde_json::Value;

pub const ENTITY_FIRMWARE_DEPLOYMENT: &str = "firmware_deployment";
const DEPLOYMENT_STATE_PENDING: &str = "pending";

pub fn resolve_rollout_target_device_ids(
    target_policy_json: &str,
    devices: &[AiotDeviceRecord],
) -> Vec<String> {
    let Ok(policy) = serde_json::from_str::<Value>(target_policy_json) else {
        return Vec::new();
    };

    let scope = policy.get("scope").and_then(Value::as_str).unwrap_or("all");
    let batch = policy
        .get("batch")
        .and_then(Value::as_u64)
        .map(|value| value as usize);

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
            devices
                .iter()
                .filter(|device| device.product_id == product_id)
                .map(|device| device.device_id.clone())
                .collect()
        }
        _ => devices
            .iter()
            .map(|device| device.device_id.clone())
            .collect(),
    };

    candidates.sort();
    candidates.dedup();

    if let Some(limit) = batch {
        candidates.truncate(limit);
    }

    candidates
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
    use sdkwork_aiot_storage::AiotDeviceRecord;

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

    #[test]
    fn resolve_all_scope_honors_batch_limit() {
        let devices = vec![
            sample_device("device-b", "1009"),
            sample_device("device-a", "1009"),
            sample_device("device-c", "1009"),
        ];
        let targets = resolve_rollout_target_device_ids(r#"{"scope":"all","batch":2}"#, &devices);
        assert_eq!(
            targets,
            vec!["device-a".to_string(), "device-b".to_string()]
        );
    }

    #[test]
    fn resolve_devices_scope_uses_explicit_ids() {
        let devices = vec![sample_device("device-a", "1009")];
        let targets = resolve_rollout_target_device_ids(
            r#"{"scope":"devices","deviceIds":["device-x","device-a"]}"#,
            &devices,
        );
        assert_eq!(
            targets,
            vec!["device-a".to_string(), "device-x".to_string()]
        );
    }
}
