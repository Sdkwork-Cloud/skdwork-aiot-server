use std::sync::Arc;

use sdkwork_aiot_storage::{
    paginate_vec, AiotOffsetListResult, AiotStorageAssociation, OffsetListPageParams,
};
use sdkwork_aiot_storage_sqlx::SqlitePersistedEntityRepository;
use sdkwork_iot_device_service::{CapabilityDefinition, CapabilityKind};

use crate::{
    AiotCapabilityModelCreatePayload, AiotCapabilityModelRecord, AiotCapabilityModelUpdatePayload,
    AiotCatalogRepositoryError, AiotFirmwareArtifactCreatePayload, AiotFirmwareArtifactRecord,
    AiotFirmwareArtifactUpdatePayload, AiotFirmwareRepositoryError,
    AiotFirmwareRolloutCreatePayload, AiotFirmwareRolloutRecord, AiotFirmwareRolloutUpdatePayload,
    AiotHardwareProfileCreatePayload, AiotHardwareProfileRecord, AiotHardwareProfileUpdatePayload,
    AiotProductCreatePayload, AiotProductRecord, AiotProductUpdatePayload,
    AiotProtocolProfileCreatePayload, AiotProtocolProfileRecord, AiotProtocolProfileUpdatePayload,
    InMemoryAiotCatalogRepository, InMemoryAiotFirmwareRepository,
};

const ENTITY_PRODUCT: &str = "product";
const ENTITY_HARDWARE_PROFILE: &str = "hardware_profile";
const ENTITY_PROTOCOL_PROFILE: &str = "protocol_profile";
const ENTITY_CAPABILITY_MODEL: &str = "capability_model";
const ENTITY_FIRMWARE_ARTIFACT: &str = "firmware_artifact";
const ENTITY_FIRMWARE_ROLLOUT: &str = "firmware_rollout";

use crate::firmware_rollout_planner::{
    firmware_deployment_payload_json, rollout_force_from_policy, ENTITY_FIRMWARE_DEPLOYMENT,
};

fn map_persisted_entity_page<T, F>(
    page: Result<
        AiotOffsetListResult<sdkwork_aiot_storage_sqlx::SqlitePersistedEntityRecord>,
        sdkwork_aiot_storage_sqlx::SqlitePersistedEntityError,
    >,
    parse: F,
) -> Result<AiotOffsetListResult<T>, AiotCatalogRepositoryError>
where
    F: Fn(&str) -> Option<T>,
{
    match page {
        Ok(page) => Ok(AiotOffsetListResult {
            items: page
                .items
                .into_iter()
                .filter_map(|entity| parse(&entity.payload_json))
                .collect(),
            total: page.total,
        }),
        Err(_) => Err(AiotCatalogRepositoryError::StorageFailure),
    }
}

fn map_persisted_entity_page_firmware<T, F>(
    page: Result<
        AiotOffsetListResult<sdkwork_aiot_storage_sqlx::SqlitePersistedEntityRecord>,
        sdkwork_aiot_storage_sqlx::SqlitePersistedEntityError,
    >,
    parse: F,
) -> Result<AiotOffsetListResult<T>, AiotFirmwareRepositoryError>
where
    F: Fn(&str) -> Option<T>,
{
    match page {
        Ok(page) => Ok(AiotOffsetListResult {
            items: page
                .items
                .into_iter()
                .filter_map(|entity| parse(&entity.payload_json))
                .collect(),
            total: page.total,
        }),
        Err(_) => Err(AiotFirmwareRepositoryError::StorageFailure),
    }
}

#[derive(Clone)]
pub struct AiotCatalogRepositoryHandle {
    memory: InMemoryAiotCatalogRepository,
    store: Option<Arc<SqlitePersistedEntityRepository>>,
}

impl AiotCatalogRepositoryHandle {
    pub fn new_in_memory() -> Self {
        Self {
            memory: InMemoryAiotCatalogRepository::new(),
            store: None,
        }
    }

    pub fn open_sqlite(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        Ok(Self::from_entity_store(Arc::new(
            SqlitePersistedEntityRepository::open(path).map_err(|error| error.to_string())?,
        )))
    }

    pub fn from_entity_store(store: Arc<SqlitePersistedEntityRepository>) -> Self {
        Self {
            memory: InMemoryAiotCatalogRepository::new(),
            store: Some(store),
        }
    }

    pub fn create_product(
        &self,
        association: AiotStorageAssociation,
        payload: AiotProductCreatePayload,
    ) -> Result<AiotProductRecord, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            if store
                .get_entity(&association, ENTITY_PRODUCT, &payload.product_id)
                .is_some()
            {
                return Err(AiotCatalogRepositoryError::DuplicateProductId);
            }
            let record = AiotProductRecord {
                product_id: payload.product_id.clone(),
                display_name: payload.display_name,
                default_hardware_profile_id: payload.default_hardware_profile_id,
                default_protocol_profile_id: payload.default_protocol_profile_id,
                default_capability_model_id: payload.default_capability_model_id,
                status: "active".to_string(),
            };
            store
                .upsert_entity(
                    &association,
                    ENTITY_PRODUCT,
                    &record.product_id,
                    &product_record_json(&record),
                )
                .map_err(|_| AiotCatalogRepositoryError::ProductNotFound)?;
            return Ok(record);
        }
        self.memory.create_product(association, payload)
    }

    pub fn get_product(
        &self,
        association: &AiotStorageAssociation,
        product_id: &str,
    ) -> Option<AiotProductRecord> {
        if let Some(store) = &self.store {
            return store
                .get_entity(association, ENTITY_PRODUCT, product_id)
                .and_then(|entity| parse_product_record(&entity.payload_json));
        }
        self.memory.get_product(association, product_id)
    }

    pub fn list_products(&self, association: &AiotStorageAssociation) -> Vec<AiotProductRecord> {
        if let Some(store) = &self.store {
            return store
                .list_entities(association, ENTITY_PRODUCT)
                .into_iter()
                .filter_map(|entity| parse_product_record(&entity.payload_json))
                .collect();
        }
        self.memory.list_products(association)
    }

    pub fn list_products_page(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotProductRecord>, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            return map_persisted_entity_page(
                store.list_entities_page(association, ENTITY_PRODUCT, params),
                parse_product_record,
            );
        }
        Ok(paginate_vec(self.memory.list_products(association), params))
    }

    pub fn update_product(
        &self,
        association: AiotStorageAssociation,
        product_id: &str,
        payload: AiotProductUpdatePayload,
    ) -> Result<AiotProductRecord, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            let Some(mut record) = self.get_product(&association, product_id) else {
                return Err(AiotCatalogRepositoryError::ProductNotFound);
            };
            if let Some(display_name) = payload.display_name {
                record.display_name = display_name;
            }
            if let Some(default_hardware_profile_id) = payload.default_hardware_profile_id {
                record.default_hardware_profile_id = default_hardware_profile_id;
            }
            if let Some(default_protocol_profile_id) = payload.default_protocol_profile_id {
                record.default_protocol_profile_id = default_protocol_profile_id;
            }
            if let Some(default_capability_model_id) = payload.default_capability_model_id {
                record.default_capability_model_id = default_capability_model_id;
            }
            if let Some(status) = payload.status {
                record.status = status;
            }
            store
                .upsert_entity(
                    &association,
                    ENTITY_PRODUCT,
                    product_id,
                    &product_record_json(&record),
                )
                .map_err(|_| AiotCatalogRepositoryError::ProductNotFound)?;
            return Ok(record);
        }
        self.memory.update_product(association, product_id, payload)
    }

    pub fn delete_product(
        &self,
        association: &AiotStorageAssociation,
        product_id: &str,
    ) -> Result<(), AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            store
                .delete_entity(association, ENTITY_PRODUCT, product_id)
                .map_err(|error| match error {
                    sdkwork_aiot_storage_sqlx::SqlitePersistedEntityError::NotFound => {
                        AiotCatalogRepositoryError::ProductNotFound
                    }
                    _ => AiotCatalogRepositoryError::ProductNotFound,
                })?;
            return Ok(());
        }
        self.memory.delete_product(association, product_id)
    }

    pub fn create_hardware_profile(
        &self,
        association: AiotStorageAssociation,
        payload: AiotHardwareProfileCreatePayload,
    ) -> Result<AiotHardwareProfileRecord, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            if store
                .get_entity(
                    &association,
                    ENTITY_HARDWARE_PROFILE,
                    &payload.hardware_profile_id,
                )
                .is_some()
            {
                return Err(AiotCatalogRepositoryError::DuplicateHardwareProfileId);
            }
            let record = AiotHardwareProfileRecord {
                hardware_profile_id: payload.hardware_profile_id.clone(),
                chip_family: payload.chip_family,
                hardware_classes: payload.hardware_classes,
                runtime_profiles: payload.runtime_profiles,
                connectivity_profiles: payload.connectivity_profiles,
                security_profiles: payload.security_profiles,
                ota_profiles: payload.ota_profiles,
                status: "active".to_string(),
            };
            store
                .upsert_entity(
                    &association,
                    ENTITY_HARDWARE_PROFILE,
                    &record.hardware_profile_id,
                    &hardware_profile_record_json(&record),
                )
                .map_err(|_| AiotCatalogRepositoryError::HardwareProfileNotFound)?;
            return Ok(record);
        }
        self.memory.create_hardware_profile(association, payload)
    }

    pub fn get_hardware_profile(
        &self,
        association: &AiotStorageAssociation,
        hardware_profile_id: &str,
    ) -> Option<AiotHardwareProfileRecord> {
        if let Some(store) = &self.store {
            return store
                .get_entity(association, ENTITY_HARDWARE_PROFILE, hardware_profile_id)
                .and_then(|entity| parse_hardware_profile_record(&entity.payload_json));
        }
        self.memory
            .get_hardware_profile(association, hardware_profile_id)
    }

    pub fn list_hardware_profiles(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotHardwareProfileRecord> {
        if let Some(store) = &self.store {
            return store
                .list_entities(association, ENTITY_HARDWARE_PROFILE)
                .into_iter()
                .filter_map(|entity| parse_hardware_profile_record(&entity.payload_json))
                .collect();
        }
        self.memory.list_hardware_profiles(association)
    }

    pub fn list_hardware_profiles_page(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotHardwareProfileRecord>, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            return map_persisted_entity_page(
                store.list_entities_page(association, ENTITY_HARDWARE_PROFILE, params),
                parse_hardware_profile_record,
            );
        }
        Ok(paginate_vec(
            self.memory.list_hardware_profiles(association),
            params,
        ))
    }

    pub fn update_hardware_profile(
        &self,
        association: AiotStorageAssociation,
        hardware_profile_id: &str,
        payload: AiotHardwareProfileUpdatePayload,
    ) -> Result<AiotHardwareProfileRecord, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            let Some(mut record) = self.get_hardware_profile(&association, hardware_profile_id)
            else {
                return Err(AiotCatalogRepositoryError::HardwareProfileNotFound);
            };
            if let Some(chip_family) = payload.chip_family {
                record.chip_family = chip_family;
            }
            if let Some(hardware_classes) = payload.hardware_classes {
                record.hardware_classes = hardware_classes;
            }
            if let Some(runtime_profiles) = payload.runtime_profiles {
                record.runtime_profiles = runtime_profiles;
            }
            if let Some(connectivity_profiles) = payload.connectivity_profiles {
                record.connectivity_profiles = connectivity_profiles;
            }
            if let Some(security_profiles) = payload.security_profiles {
                record.security_profiles = security_profiles;
            }
            if let Some(ota_profiles) = payload.ota_profiles {
                record.ota_profiles = ota_profiles;
            }
            if let Some(status) = payload.status {
                record.status = status;
            }
            store
                .upsert_entity(
                    &association,
                    ENTITY_HARDWARE_PROFILE,
                    hardware_profile_id,
                    &hardware_profile_record_json(&record),
                )
                .map_err(|_| AiotCatalogRepositoryError::HardwareProfileNotFound)?;
            return Ok(record);
        }
        self.memory
            .update_hardware_profile(association, hardware_profile_id, payload)
    }

    pub fn delete_hardware_profile(
        &self,
        association: &AiotStorageAssociation,
        hardware_profile_id: &str,
    ) -> Result<(), AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            store
                .delete_entity(association, ENTITY_HARDWARE_PROFILE, hardware_profile_id)
                .map_err(|_| AiotCatalogRepositoryError::HardwareProfileNotFound)?;
            return Ok(());
        }
        self.memory
            .delete_hardware_profile(association, hardware_profile_id)
    }

    pub fn create_protocol_profile(
        &self,
        association: AiotStorageAssociation,
        payload: AiotProtocolProfileCreatePayload,
    ) -> Result<AiotProtocolProfileRecord, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            if store
                .get_entity(
                    &association,
                    ENTITY_PROTOCOL_PROFILE,
                    &payload.protocol_profile_id,
                )
                .is_some()
            {
                return Err(AiotCatalogRepositoryError::DuplicateProtocolProfileId);
            }
            let record = AiotProtocolProfileRecord {
                protocol_profile_id: payload.protocol_profile_id.clone(),
                default_protocol_id: payload.default_protocol_id,
                scope: payload.scope,
                allowed_transports: payload.allowed_transports,
                allowed_message_classes: payload.allowed_message_classes,
                capability_bridges: payload.capability_bridges,
                status: "active".to_string(),
            };
            store
                .upsert_entity(
                    &association,
                    ENTITY_PROTOCOL_PROFILE,
                    &record.protocol_profile_id,
                    &protocol_profile_record_json(&record),
                )
                .map_err(|_| AiotCatalogRepositoryError::ProtocolProfileNotFound)?;
            return Ok(record);
        }
        self.memory.create_protocol_profile(association, payload)
    }

    pub fn get_protocol_profile(
        &self,
        association: &AiotStorageAssociation,
        protocol_profile_id: &str,
    ) -> Option<AiotProtocolProfileRecord> {
        if let Some(store) = &self.store {
            return store
                .get_entity(association, ENTITY_PROTOCOL_PROFILE, protocol_profile_id)
                .and_then(|entity| parse_protocol_profile_record(&entity.payload_json));
        }
        self.memory
            .get_protocol_profile(association, protocol_profile_id)
    }

    pub fn list_protocol_profiles(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotProtocolProfileRecord> {
        if let Some(store) = &self.store {
            return store
                .list_entities(association, ENTITY_PROTOCOL_PROFILE)
                .into_iter()
                .filter_map(|entity| parse_protocol_profile_record(&entity.payload_json))
                .collect();
        }
        self.memory.list_protocol_profiles(association)
    }

    pub fn list_protocol_profiles_page(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotProtocolProfileRecord>, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            return map_persisted_entity_page(
                store.list_entities_page(association, ENTITY_PROTOCOL_PROFILE, params),
                parse_protocol_profile_record,
            );
        }
        Ok(paginate_vec(
            self.memory.list_protocol_profiles(association),
            params,
        ))
    }

    pub fn update_protocol_profile(
        &self,
        association: AiotStorageAssociation,
        protocol_profile_id: &str,
        payload: AiotProtocolProfileUpdatePayload,
    ) -> Result<AiotProtocolProfileRecord, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            let Some(mut record) = self.get_protocol_profile(&association, protocol_profile_id)
            else {
                return Err(AiotCatalogRepositoryError::ProtocolProfileNotFound);
            };
            if let Some(default_protocol_id) = payload.default_protocol_id {
                record.default_protocol_id = default_protocol_id;
            }
            if let Some(scope) = payload.scope {
                record.scope = scope;
            }
            if let Some(allowed_transports) = payload.allowed_transports {
                record.allowed_transports = allowed_transports;
            }
            if let Some(allowed_message_classes) = payload.allowed_message_classes {
                record.allowed_message_classes = allowed_message_classes;
            }
            if let Some(capability_bridges) = payload.capability_bridges {
                record.capability_bridges = capability_bridges;
            }
            if let Some(status) = payload.status {
                record.status = status;
            }
            store
                .upsert_entity(
                    &association,
                    ENTITY_PROTOCOL_PROFILE,
                    protocol_profile_id,
                    &protocol_profile_record_json(&record),
                )
                .map_err(|_| AiotCatalogRepositoryError::ProtocolProfileNotFound)?;
            return Ok(record);
        }
        self.memory
            .update_protocol_profile(association, protocol_profile_id, payload)
    }

    pub fn delete_protocol_profile(
        &self,
        association: &AiotStorageAssociation,
        protocol_profile_id: &str,
    ) -> Result<(), AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            store
                .delete_entity(association, ENTITY_PROTOCOL_PROFILE, protocol_profile_id)
                .map_err(|_| AiotCatalogRepositoryError::ProtocolProfileNotFound)?;
            return Ok(());
        }
        self.memory
            .delete_protocol_profile(association, protocol_profile_id)
    }

    pub fn create_capability_model(
        &self,
        association: AiotStorageAssociation,
        payload: AiotCapabilityModelCreatePayload,
    ) -> Result<AiotCapabilityModelRecord, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            if store
                .get_entity(
                    &association,
                    ENTITY_CAPABILITY_MODEL,
                    &payload.capability_model_id,
                )
                .is_some()
            {
                return Err(AiotCatalogRepositoryError::DuplicateCapabilityModelId);
            }
            let record = AiotCapabilityModelRecord {
                capability_model_id: payload.capability_model_id.clone(),
                display_name: payload.display_name,
                version: payload.version,
                capabilities: payload.capabilities,
                status: "active".to_string(),
            };
            store
                .upsert_entity(
                    &association,
                    ENTITY_CAPABILITY_MODEL,
                    &record.capability_model_id,
                    &capability_model_record_json(&record),
                )
                .map_err(|_| AiotCatalogRepositoryError::CapabilityModelNotFound)?;
            return Ok(record);
        }
        self.memory.create_capability_model(association, payload)
    }

    pub fn get_capability_model(
        &self,
        association: &AiotStorageAssociation,
        capability_model_id: &str,
    ) -> Option<AiotCapabilityModelRecord> {
        if let Some(store) = &self.store {
            return store
                .get_entity(association, ENTITY_CAPABILITY_MODEL, capability_model_id)
                .and_then(|entity| parse_capability_model_record(&entity.payload_json));
        }
        self.memory
            .get_capability_model(association, capability_model_id)
    }

    pub fn get_seed_capability_model(
        &self,
        capability_model_id: &str,
    ) -> Option<AiotCapabilityModelRecord> {
        self.memory.get_seed_capability_model(capability_model_id)
    }

    pub fn list_capability_models(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotCapabilityModelRecord> {
        if let Some(store) = &self.store {
            return store
                .list_entities(association, ENTITY_CAPABILITY_MODEL)
                .into_iter()
                .filter_map(|entity| parse_capability_model_record(&entity.payload_json))
                .collect();
        }
        self.memory.list_capability_models(association)
    }

    pub fn list_capability_models_page(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotCapabilityModelRecord>, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            return map_persisted_entity_page(
                store.list_entities_page(association, ENTITY_CAPABILITY_MODEL, params),
                parse_capability_model_record,
            );
        }
        Ok(paginate_vec(
            self.memory.list_capability_models(association),
            params,
        ))
    }

    pub fn update_capability_model(
        &self,
        association: AiotStorageAssociation,
        capability_model_id: &str,
        payload: AiotCapabilityModelUpdatePayload,
    ) -> Result<AiotCapabilityModelRecord, AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            let Some(mut record) = self.get_capability_model(&association, capability_model_id)
            else {
                return Err(AiotCatalogRepositoryError::CapabilityModelNotFound);
            };
            if let Some(display_name) = payload.display_name {
                record.display_name = display_name;
            }
            if let Some(version) = payload.version {
                record.version = version;
            }
            if let Some(capabilities) = payload.capabilities {
                record.capabilities = capabilities;
            }
            if let Some(status) = payload.status {
                record.status = status;
            }
            store
                .upsert_entity(
                    &association,
                    ENTITY_CAPABILITY_MODEL,
                    capability_model_id,
                    &capability_model_record_json(&record),
                )
                .map_err(|_| AiotCatalogRepositoryError::CapabilityModelNotFound)?;
            return Ok(record);
        }
        self.memory
            .update_capability_model(association, capability_model_id, payload)
    }

    pub fn delete_capability_model(
        &self,
        association: &AiotStorageAssociation,
        capability_model_id: &str,
    ) -> Result<(), AiotCatalogRepositoryError> {
        if let Some(store) = &self.store {
            store
                .delete_entity(association, ENTITY_CAPABILITY_MODEL, capability_model_id)
                .map_err(|_| AiotCatalogRepositoryError::CapabilityModelNotFound)?;
            return Ok(());
        }
        self.memory
            .delete_capability_model(association, capability_model_id)
    }
}

#[derive(Clone)]
pub struct AiotFirmwareRepositoryHandle {
    memory: InMemoryAiotFirmwareRepository,
    store: Option<Arc<SqlitePersistedEntityRepository>>,
}

impl AiotFirmwareRepositoryHandle {
    pub fn new_in_memory() -> Self {
        Self {
            memory: InMemoryAiotFirmwareRepository::new(),
            store: None,
        }
    }

    pub fn open_sqlite(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        Ok(Self::from_entity_store(Arc::new(
            SqlitePersistedEntityRepository::open(path).map_err(|error| error.to_string())?,
        )))
    }

    pub fn from_entity_store(store: Arc<SqlitePersistedEntityRepository>) -> Self {
        Self {
            memory: InMemoryAiotFirmwareRepository::new(),
            store: Some(store),
        }
    }

    pub fn create_artifact(
        &self,
        association: AiotStorageAssociation,
        payload: AiotFirmwareArtifactCreatePayload,
    ) -> Result<AiotFirmwareArtifactRecord, AiotFirmwareRepositoryError> {
        if let Some(store) = &self.store {
            let artifact_id = next_firmware_artifact_id(store, &association);
            let record = build_firmware_artifact_record(&association, &artifact_id, payload);
            store
                .upsert_entity(
                    &association,
                    ENTITY_FIRMWARE_ARTIFACT,
                    &artifact_id,
                    &firmware_artifact_json(&record),
                )
                .map_err(|_| AiotFirmwareRepositoryError::ArtifactNotFound)?;
            return Ok(record);
        }
        self.memory.create_artifact(association, payload)
    }

    pub fn list_artifacts(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotFirmwareArtifactRecord> {
        if let Some(store) = &self.store {
            return store
                .list_entities(association, ENTITY_FIRMWARE_ARTIFACT)
                .into_iter()
                .filter_map(|entity| parse_firmware_artifact(&entity.payload_json))
                .collect();
        }
        self.memory.list_artifacts(association)
    }

    pub fn list_artifacts_page(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotFirmwareArtifactRecord>, AiotFirmwareRepositoryError> {
        if let Some(store) = &self.store {
            return map_persisted_entity_page_firmware(
                store.list_entities_page(association, ENTITY_FIRMWARE_ARTIFACT, params),
                parse_firmware_artifact,
            );
        }
        Ok(paginate_vec(
            self.memory.list_artifacts(association),
            params,
        ))
    }

    pub fn get_artifact(
        &self,
        association: &AiotStorageAssociation,
        artifact_id: &str,
    ) -> Option<AiotFirmwareArtifactRecord> {
        if let Some(store) = &self.store {
            return store
                .get_entity(association, ENTITY_FIRMWARE_ARTIFACT, artifact_id)
                .and_then(|entity| parse_firmware_artifact(&entity.payload_json));
        }
        self.memory.get_artifact(association, artifact_id)
    }

    pub fn update_artifact(
        &self,
        association: AiotStorageAssociation,
        artifact_id: &str,
        payload: AiotFirmwareArtifactUpdatePayload,
    ) -> Result<AiotFirmwareArtifactRecord, AiotFirmwareRepositoryError> {
        if let Some(store) = &self.store {
            let Some(mut record) = self.get_artifact(&association, artifact_id) else {
                return Err(AiotFirmwareRepositoryError::ArtifactNotFound);
            };
            apply_firmware_artifact_update(&mut record, payload);
            store
                .upsert_entity(
                    &association,
                    ENTITY_FIRMWARE_ARTIFACT,
                    artifact_id,
                    &firmware_artifact_json(&record),
                )
                .map_err(|_| AiotFirmwareRepositoryError::ArtifactNotFound)?;
            return Ok(record);
        }
        self.memory
            .update_artifact(association, artifact_id, payload)
    }

    pub fn delete_artifact(
        &self,
        association: &AiotStorageAssociation,
        artifact_id: &str,
    ) -> Result<(), AiotFirmwareRepositoryError> {
        if let Some(store) = &self.store {
            store
                .delete_entity(association, ENTITY_FIRMWARE_ARTIFACT, artifact_id)
                .map_err(|_| AiotFirmwareRepositoryError::ArtifactNotFound)?;
            return Ok(());
        }
        self.memory.delete_artifact(association, artifact_id)
    }

    pub fn create_rollout(
        &self,
        association: AiotStorageAssociation,
        payload: AiotFirmwareRolloutCreatePayload,
        target_device_ids: &[String],
    ) -> Result<AiotFirmwareRolloutRecord, AiotFirmwareRepositoryError> {
        if let Some(store) = &self.store {
            if self
                .get_artifact(&association, &payload.artifact_id)
                .is_none()
            {
                return Err(AiotFirmwareRepositoryError::InvalidReference);
            }
            let rollout_id = next_firmware_rollout_id(store, &association);
            let record = AiotFirmwareRolloutRecord {
                rollout_id: rollout_id.clone(),
                tenant_id: association.tenant_id,
                organization_id: association.organization_id,
                artifact_id: payload.artifact_id.clone(),
                target_policy_json: payload.target_policy_json.clone(),
                status: "accepted".to_string(),
            };
            store
                .upsert_entity(
                    &association,
                    ENTITY_FIRMWARE_ROLLOUT,
                    &rollout_id,
                    &firmware_rollout_json(&record),
                )
                .map_err(|_| AiotFirmwareRepositoryError::RolloutNotFound)?;
            plan_firmware_deployments(
                store.as_ref(),
                &association,
                &rollout_id,
                &payload.artifact_id,
                &payload.target_policy_json,
                target_device_ids,
            );
            return Ok(record);
        }
        self.memory
            .create_rollout(association, payload, target_device_ids)
    }

    pub fn list_rollouts(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotFirmwareRolloutRecord> {
        if let Some(store) = &self.store {
            return store
                .list_entities(association, ENTITY_FIRMWARE_ROLLOUT)
                .into_iter()
                .filter_map(|entity| parse_firmware_rollout(&entity.payload_json))
                .collect();
        }
        self.memory.list_rollouts(association)
    }

    pub fn list_rollouts_page(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotFirmwareRolloutRecord>, AiotFirmwareRepositoryError> {
        if let Some(store) = &self.store {
            return map_persisted_entity_page_firmware(
                store.list_entities_page(association, ENTITY_FIRMWARE_ROLLOUT, params),
                parse_firmware_rollout,
            );
        }
        Ok(paginate_vec(self.memory.list_rollouts(association), params))
    }

    pub fn get_rollout(
        &self,
        association: &AiotStorageAssociation,
        rollout_id: &str,
    ) -> Option<AiotFirmwareRolloutRecord> {
        if let Some(store) = &self.store {
            return store
                .get_entity(association, ENTITY_FIRMWARE_ROLLOUT, rollout_id)
                .and_then(|entity| parse_firmware_rollout(&entity.payload_json));
        }
        self.memory.get_rollout(association, rollout_id)
    }

    pub fn update_rollout(
        &self,
        association: AiotStorageAssociation,
        rollout_id: &str,
        payload: AiotFirmwareRolloutUpdatePayload,
    ) -> Result<AiotFirmwareRolloutRecord, AiotFirmwareRepositoryError> {
        if let Some(store) = &self.store {
            let Some(mut record) = self.get_rollout(&association, rollout_id) else {
                return Err(AiotFirmwareRepositoryError::RolloutNotFound);
            };
            if let Some(target_policy_json) = payload.target_policy_json {
                record.target_policy_json = target_policy_json;
            }
            if let Some(status) = payload.status {
                record.status = status;
            }
            store
                .upsert_entity(
                    &association,
                    ENTITY_FIRMWARE_ROLLOUT,
                    rollout_id,
                    &firmware_rollout_json(&record),
                )
                .map_err(|_| AiotFirmwareRepositoryError::RolloutNotFound)?;
            return Ok(record);
        }
        self.memory.update_rollout(association, rollout_id, payload)
    }

    pub fn delete_rollout(
        &self,
        association: &AiotStorageAssociation,
        rollout_id: &str,
    ) -> Result<(), AiotFirmwareRepositoryError> {
        if let Some(store) = &self.store {
            store
                .delete_entity(association, ENTITY_FIRMWARE_ROLLOUT, rollout_id)
                .map_err(|_| AiotFirmwareRepositoryError::RolloutNotFound)?;
            return Ok(());
        }
        self.memory.delete_rollout(association, rollout_id)
    }
}

fn next_firmware_artifact_id(
    store: &SqlitePersistedEntityRepository,
    association: &AiotStorageAssociation,
) -> String {
    let next = store
        .list_entities(association, ENTITY_FIRMWARE_ARTIFACT)
        .len()
        + 1;
    format!("firmware-artifact-{next:04}")
}

fn next_firmware_rollout_id(
    store: &SqlitePersistedEntityRepository,
    association: &AiotStorageAssociation,
) -> String {
    let next = store
        .list_entities(association, ENTITY_FIRMWARE_ROLLOUT)
        .len()
        + 1;
    format!("firmware-rollout-{next:04}")
}

fn next_firmware_deployment_id(
    store: &SqlitePersistedEntityRepository,
    association: &AiotStorageAssociation,
) -> String {
    let next = store
        .list_entities(association, ENTITY_FIRMWARE_DEPLOYMENT)
        .len()
        + 1;
    format!("firmware-deployment-{next:04}")
}

fn plan_firmware_deployments(
    store: &SqlitePersistedEntityRepository,
    association: &AiotStorageAssociation,
    rollout_id: &str,
    artifact_id: &str,
    target_policy_json: &str,
    target_device_ids: &[String],
) {
    let force = rollout_force_from_policy(target_policy_json);
    for device_id in target_device_ids {
        let deployment_id = next_firmware_deployment_id(store, association);
        let payload_json = firmware_deployment_payload_json(
            &deployment_id,
            association,
            rollout_id,
            artifact_id,
            device_id,
            force,
        );
        let _ = store.upsert_entity(
            association,
            ENTITY_FIRMWARE_DEPLOYMENT,
            &deployment_id,
            &payload_json,
        );
    }
}

fn build_firmware_artifact_record(
    association: &AiotStorageAssociation,
    artifact_id: &str,
    payload: AiotFirmwareArtifactCreatePayload,
) -> AiotFirmwareArtifactRecord {
    let resource_json = if let Some(object_blob_id) = payload.object_blob_id.as_deref() {
        crate::apply_media_object_blob_id(&payload.resource_json, object_blob_id)
            .unwrap_or_else(|_| payload.resource_json.clone())
    } else {
        payload.resource_json.clone()
    };
    AiotFirmwareArtifactRecord {
        artifact_id: artifact_id.to_string(),
        tenant_id: association.tenant_id,
        organization_id: association.organization_id,
        artifact_key: payload.artifact_key,
        version: payload.version,
        media_resource_id: payload.media_resource_id,
        resource_json,
        sha256: payload.sha256,
        signature: payload.signature,
        target_chip_family: payload.target_chip_family,
        target_runtime_profile: payload.target_runtime_profile,
        status: "active".to_string(),
    }
}

fn apply_firmware_artifact_update(
    record: &mut AiotFirmwareArtifactRecord,
    payload: AiotFirmwareArtifactUpdatePayload,
) {
    if let Some(artifact_key) = payload.artifact_key {
        record.artifact_key = artifact_key;
    }
    if let Some(version) = payload.version {
        record.version = version;
    }
    if let Some(resource_json) = payload.resource_json {
        record.resource_json = resource_json;
    }
    if let Some(media_resource_id) = payload.media_resource_id {
        record.media_resource_id = media_resource_id;
    }
    if let Some(object_blob_id) = payload.object_blob_id {
        if let Ok(resource_json) =
            crate::apply_media_object_blob_id(&record.resource_json, &object_blob_id)
        {
            record.resource_json = resource_json;
        }
    }
    if let Some(sha256) = payload.sha256 {
        record.sha256 = sha256;
    }
    if payload.signature.is_some() {
        record.signature = payload.signature;
    }
    if payload.target_chip_family.is_some() {
        record.target_chip_family = payload.target_chip_family;
    }
    if payload.target_runtime_profile.is_some() {
        record.target_runtime_profile = payload.target_runtime_profile;
    }
    if let Some(status) = payload.status {
        record.status = status;
    }
}

fn product_record_json(record: &AiotProductRecord) -> String {
    serde_json::json!({
        "productId": record.product_id,
        "displayName": record.display_name,
        "defaultHardwareProfileId": record.default_hardware_profile_id,
        "defaultProtocolProfileId": record.default_protocol_profile_id,
        "defaultCapabilityModelId": record.default_capability_model_id,
        "status": record.status,
    })
    .to_string()
}

fn parse_product_record(payload_json: &str) -> Option<AiotProductRecord> {
    let value: serde_json::Value = serde_json::from_str(payload_json).ok()?;
    Some(AiotProductRecord {
        product_id: value.get("productId")?.as_str()?.to_string(),
        display_name: value.get("displayName")?.as_str()?.to_string(),
        default_hardware_profile_id: value.get("defaultHardwareProfileId")?.as_str()?.to_string(),
        default_protocol_profile_id: value.get("defaultProtocolProfileId")?.as_str()?.to_string(),
        default_capability_model_id: value.get("defaultCapabilityModelId")?.as_str()?.to_string(),
        status: value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("active")
            .to_string(),
    })
}

fn firmware_artifact_json(record: &AiotFirmwareArtifactRecord) -> String {
    serde_json::json!({
        "artifactId": record.artifact_id,
        "tenantId": record.tenant_id,
        "organizationId": record.organization_id,
        "artifactKey": record.artifact_key,
        "version": record.version,
        "mediaResourceId": record.media_resource_id,
        "resourceJson": record.resource_json,
        "sha256": record.sha256,
        "signature": record.signature,
        "targetChipFamily": record.target_chip_family,
        "targetRuntimeProfile": record.target_runtime_profile,
        "status": record.status,
    })
    .to_string()
}

fn parse_firmware_artifact(payload_json: &str) -> Option<AiotFirmwareArtifactRecord> {
    let value: serde_json::Value = serde_json::from_str(payload_json).ok()?;
    Some(AiotFirmwareArtifactRecord {
        artifact_id: value.get("artifactId")?.as_str()?.to_string(),
        tenant_id: value.get("tenantId")?.as_i64()?,
        organization_id: value.get("organizationId")?.as_i64()?,
        artifact_key: value.get("artifactKey")?.as_str()?.to_string(),
        version: value.get("version")?.as_str()?.to_string(),
        media_resource_id: value.get("mediaResourceId")?.as_str()?.to_string(),
        resource_json: value.get("resourceJson")?.as_str()?.to_string(),
        sha256: value.get("sha256")?.as_str()?.to_string(),
        signature: value
            .get("signature")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        target_chip_family: value
            .get("targetChipFamily")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        target_runtime_profile: value
            .get("targetRuntimeProfile")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        status: value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("active")
            .to_string(),
    })
}

fn firmware_rollout_json(record: &AiotFirmwareRolloutRecord) -> String {
    serde_json::json!({
        "rolloutId": record.rollout_id,
        "tenantId": record.tenant_id,
        "organizationId": record.organization_id,
        "artifactId": record.artifact_id,
        "targetPolicyJson": record.target_policy_json,
        "status": record.status,
    })
    .to_string()
}

fn parse_firmware_rollout(payload_json: &str) -> Option<AiotFirmwareRolloutRecord> {
    let value: serde_json::Value = serde_json::from_str(payload_json).ok()?;
    Some(AiotFirmwareRolloutRecord {
        rollout_id: value.get("rolloutId")?.as_str()?.to_string(),
        tenant_id: value.get("tenantId")?.as_i64()?,
        organization_id: value.get("organizationId")?.as_i64()?,
        artifact_id: value.get("artifactId")?.as_str()?.to_string(),
        target_policy_json: value.get("targetPolicyJson")?.as_str()?.to_string(),
        status: value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("accepted")
            .to_string(),
    })
}

fn hardware_profile_record_json(record: &AiotHardwareProfileRecord) -> String {
    serde_json::json!({
        "hardwareProfileId": record.hardware_profile_id,
        "chipFamily": record.chip_family,
        "hardwareClasses": record.hardware_classes,
        "runtimeProfiles": record.runtime_profiles,
        "connectivityProfiles": record.connectivity_profiles,
        "securityProfiles": record.security_profiles,
        "otaProfiles": record.ota_profiles,
        "status": record.status,
    })
    .to_string()
}

fn parse_hardware_profile_record(payload_json: &str) -> Option<AiotHardwareProfileRecord> {
    let value: serde_json::Value = serde_json::from_str(payload_json).ok()?;
    Some(AiotHardwareProfileRecord {
        hardware_profile_id: value.get("hardwareProfileId")?.as_str()?.to_string(),
        chip_family: value.get("chipFamily")?.as_str()?.to_string(),
        hardware_classes: json_string_array(value.get("hardwareClasses")?)?,
        runtime_profiles: json_string_array(value.get("runtimeProfiles")?)?,
        connectivity_profiles: json_string_array(value.get("connectivityProfiles")?)?,
        security_profiles: json_string_array(value.get("securityProfiles")?)?,
        ota_profiles: json_string_array(value.get("otaProfiles")?)?,
        status: value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("active")
            .to_string(),
    })
}

fn protocol_profile_record_json(record: &AiotProtocolProfileRecord) -> String {
    serde_json::json!({
        "protocolProfileId": record.protocol_profile_id,
        "defaultProtocolId": record.default_protocol_id,
        "scope": record.scope,
        "allowedTransports": record.allowed_transports,
        "allowedMessageClasses": record.allowed_message_classes,
        "capabilityBridges": record.capability_bridges,
        "status": record.status,
    })
    .to_string()
}

fn parse_protocol_profile_record(payload_json: &str) -> Option<AiotProtocolProfileRecord> {
    let value: serde_json::Value = serde_json::from_str(payload_json).ok()?;
    Some(AiotProtocolProfileRecord {
        protocol_profile_id: value.get("protocolProfileId")?.as_str()?.to_string(),
        default_protocol_id: value.get("defaultProtocolId")?.as_str()?.to_string(),
        scope: value.get("scope")?.as_str()?.to_string(),
        allowed_transports: json_string_array(value.get("allowedTransports")?)?,
        allowed_message_classes: json_string_array(value.get("allowedMessageClasses")?)?,
        capability_bridges: json_string_array(value.get("capabilityBridges")?)?,
        status: value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("active")
            .to_string(),
    })
}

fn capability_model_record_json(record: &AiotCapabilityModelRecord) -> String {
    let capabilities = record
        .capabilities
        .iter()
        .map(capability_definition_json)
        .collect::<Vec<_>>();
    serde_json::json!({
        "capabilityModelId": record.capability_model_id,
        "displayName": record.display_name,
        "version": record.version,
        "capabilities": capabilities,
        "status": record.status,
    })
    .to_string()
}

fn parse_capability_model_record(payload_json: &str) -> Option<AiotCapabilityModelRecord> {
    let value: serde_json::Value = serde_json::from_str(payload_json).ok()?;
    let capabilities = value
        .get("capabilities")?
        .as_array()?
        .iter()
        .filter_map(parse_capability_definition)
        .collect();
    Some(AiotCapabilityModelRecord {
        capability_model_id: value.get("capabilityModelId")?.as_str()?.to_string(),
        display_name: value.get("displayName")?.as_str()?.to_string(),
        version: value.get("version")?.as_str()?.to_string(),
        capabilities,
        status: value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("active")
            .to_string(),
    })
}

fn capability_definition_json(definition: &CapabilityDefinition) -> serde_json::Value {
    let mappings = definition
        .protocol_mappings
        .iter()
        .map(|(protocol_id, mapped_name)| {
            serde_json::json!({
                "protocolId": protocol_id,
                "mappedName": mapped_name,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "capabilityName": definition.name,
        "capabilityKind": capability_kind_name(definition.kind),
        "commands": definition.commands,
        "events": definition.events,
        "protocolMappings": mappings,
    })
}

fn parse_capability_definition(value: &serde_json::Value) -> Option<CapabilityDefinition> {
    let obj = value.as_object()?;
    let name = obj
        .get("capabilityName")
        .or_else(|| obj.get("name"))?
        .as_str()?;
    let kind_name = obj
        .get("capabilityKind")
        .or_else(|| obj.get("kind"))?
        .as_str()?;
    let kind = capability_kind_from_name(kind_name)?;
    let mut definition = CapabilityDefinition::new(name, kind);
    if let Some(commands) = obj.get("commands").and_then(serde_json::Value::as_array) {
        for command in commands {
            if let Some(command) = command.as_str() {
                definition = definition.with_command(command);
            }
        }
    }
    if let Some(events) = obj.get("events").and_then(serde_json::Value::as_array) {
        for event in events {
            if let Some(event) = event.as_str() {
                definition = definition.with_event(event);
            }
        }
    }
    if let Some(mappings) = obj
        .get("protocolMappings")
        .and_then(serde_json::Value::as_array)
    {
        for mapping in mappings {
            let mapping = mapping.as_object()?;
            let protocol_id = mapping.get("protocolId")?.as_str()?;
            let mapped_name = mapping.get("mappedName")?.as_str()?;
            definition = definition.with_protocol_mapping(protocol_id, mapped_name);
        }
    }
    Some(definition)
}

fn capability_kind_name(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Property => "property",
        CapabilityKind::Command => "command",
        CapabilityKind::Event => "event",
        CapabilityKind::Telemetry => "telemetry",
        CapabilityKind::Media => "media",
        CapabilityKind::Ota => "ota",
    }
}

fn capability_kind_from_name(value: &str) -> Option<CapabilityKind> {
    match value {
        "property" | "Property" => Some(CapabilityKind::Property),
        "command" | "Command" => Some(CapabilityKind::Command),
        "event" | "Event" => Some(CapabilityKind::Event),
        "telemetry" | "Telemetry" => Some(CapabilityKind::Telemetry),
        "media" | "Media" => Some(CapabilityKind::Media),
        "ota" | "Ota" | "OTA" => Some(CapabilityKind::Ota),
        _ => None,
    }
}

fn json_string_array(value: &serde_json::Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect()
}
