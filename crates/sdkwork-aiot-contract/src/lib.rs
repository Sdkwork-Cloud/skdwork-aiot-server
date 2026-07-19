pub const IOT_APP_API_PREFIX: &str = "/app/v3/api/iot";
pub const IOT_BACKEND_API_PREFIX: &str = "/backend/v3/api/iot";
pub const IOT_XIAOZHI_BASE_PATH: &str = "/iot/xiaozhi";

pub const IOT_PERMISSION_DEVICES_READ: &str = "iot.devices.read";
pub const IOT_PERMISSION_DEVICES_WRITE: &str = "iot.devices.write";
pub const IOT_PERMISSION_COMMANDS_EXECUTE: &str = "iot.commands.execute";
pub const IOT_PERMISSION_FIRMWARE_WRITE: &str = "iot.firmware.write";

pub const IOT_PERMISSION_PRODUCTS_READ: &str = "iot.products.read";
pub const IOT_PERMISSION_PRODUCTS_WRITE: &str = "iot.products.write";
pub const IOT_PERMISSION_PROFILES_READ: &str = "iot.profiles.read";
pub const IOT_PERMISSION_PROFILES_WRITE: &str = "iot.profiles.write";
pub const IOT_PERMISSION_DEVICES_BIND: &str = "iot.devices.bind";
pub const IOT_PERMISSION_DEVICES_DELETE: &str = "iot.devices.delete";
pub const IOT_PERMISSION_SESSIONS_READ: &str = "iot.sessions.read";
pub const IOT_PERMISSION_SESSIONS_DISCONNECT: &str = "iot.sessions.disconnect";
pub const IOT_PERMISSION_COMMANDS_READ: &str = "iot.commands.read";
pub const IOT_PERMISSION_COMMANDS_CANCEL: &str = "iot.commands.cancel";
pub const IOT_PERMISSION_TWINS_READ: &str = "iot.twins.read";
pub const IOT_PERMISSION_TWINS_WRITE: &str = "iot.twins.write";
pub const IOT_PERMISSION_TELEMETRY_READ: &str = "iot.telemetry.read";
pub const IOT_PERMISSION_FIRMWARE_READ: &str = "iot.firmware.read";
pub const IOT_PERMISSION_FIRMWARE_ROLLOUT: &str = "iot.firmware.rollout";
pub const IOT_PERMISSION_PROTOCOL_ADAPTERS_READ: &str = "iot.protocolAdapters.read";
pub const IOT_PERMISSION_PROTOCOL_ADAPTERS_WRITE: &str = "iot.protocolAdapters.write";
pub const IOT_PERMISSION_RUNTIME_READ: &str = "iot.runtime.read";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiotMediaKind {
    Image,
    Video,
    Audio,
    Voice,
    Document,
    Archive,
    Model,
    Other,
}

impl AiotMediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Voice => "voice",
            Self::Document => "document",
            Self::Archive => "archive",
            Self::Model => "model",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiotMediaSource {
    ObjectStorage,
    ExternalUrl,
    DataUrl,
    ProviderAsset,
    Generated,
}

impl AiotMediaSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ObjectStorage => "object_storage",
            Self::ExternalUrl => "external_url",
            Self::DataUrl => "data_url",
            Self::ProviderAsset => "provider_asset",
            Self::Generated => "generated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotMediaChecksum {
    pub algorithm: String,
    pub value: String,
}

impl AiotMediaChecksum {
    pub fn new(algorithm: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            algorithm: algorithm.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotMediaAccess {
    pub visibility: String,
    pub expires_at: Option<String>,
}

impl AiotMediaAccess {
    pub fn new(visibility: impl Into<String>) -> Self {
        Self {
            visibility: visibility.into(),
            expires_at: None,
        }
    }

    pub fn with_expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = Some(expires_at.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotMediaAiProvenance {
    pub provenance: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prompt_id: Option<String>,
    pub generation_task_id: Option<String>,
    pub source_media_ids: Vec<String>,
    pub seed: Option<String>,
    pub moderation_status: Option<String>,
    pub safety_labels: Vec<String>,
}

impl AiotMediaAiProvenance {
    pub fn generated(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provenance: Some("generated".to_string()),
            provider: Some(provider.into()),
            model: Some(model.into()),
            prompt_id: None,
            generation_task_id: None,
            source_media_ids: Vec::new(),
            seed: None,
            moderation_status: None,
            safety_labels: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotMediaResource {
    pub id: Option<String>,
    pub kind: AiotMediaKind,
    pub source: AiotMediaSource,
    pub url: Option<String>,
    pub public_url: Option<String>,
    pub uri: Option<String>,
    pub object_blob_id: Option<String>,
    pub bucket_id: Option<String>,
    pub object_key: Option<String>,
    pub object_version: Option<String>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<String>,
    pub checksum: Option<AiotMediaChecksum>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<String>,
    pub alt_text: Option<String>,
    pub title: Option<String>,
    pub poster: Option<Box<AiotMediaResource>>,
    pub thumbnails: Vec<AiotMediaResource>,
    pub variants: Vec<AiotMediaResource>,
    pub access: Option<AiotMediaAccess>,
    pub ai: Option<AiotMediaAiProvenance>,
    pub metadata_json: Option<String>,
}

impl AiotMediaResource {
    pub fn new(kind: AiotMediaKind, source: AiotMediaSource) -> Self {
        Self {
            id: None,
            kind,
            source,
            url: None,
            public_url: None,
            uri: None,
            object_blob_id: None,
            bucket_id: None,
            object_key: None,
            object_version: None,
            file_name: None,
            mime_type: None,
            size_bytes: None,
            checksum: None,
            width: None,
            height: None,
            duration_seconds: None,
            alt_text: None,
            title: None,
            poster: None,
            thumbnails: Vec::new(),
            variants: Vec::new(),
            access: None,
            ai: None,
            metadata_json: None,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_object_storage_identity(
        mut self,
        object_blob_id: impl Into<String>,
        bucket_id: impl Into<String>,
        object_key: impl Into<String>,
    ) -> Self {
        self.object_blob_id = Some(object_blob_id.into());
        self.bucket_id = Some(bucket_id.into());
        self.object_key = Some(object_key.into());
        self
    }

    pub fn with_delivery_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotActorRef {
    pub actor_type: String,
    pub actor_id: String,
}

impl AiotActorRef {
    pub fn new(actor_type: impl Into<String>, actor_id: impl Into<String>) -> Self {
        Self {
            actor_type: actor_type.into(),
            actor_id: actor_id.into(),
        }
    }

    pub fn iam_user(user_id: impl Into<String>) -> Self {
        Self::new("iam_user", user_id)
    }

    pub fn iam_service(service_id: impl Into<String>) -> Self {
        Self::new("iam_service", service_id)
    }

    pub fn device(device_id: impl Into<String>) -> Self {
        Self::new("device", device_id)
    }

    pub fn system() -> Self {
        Self::new("system", "system")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotOwnershipRef {
    pub owner_type: String,
    pub owner_id: String,
}

impl AiotOwnershipRef {
    pub fn new(owner_type: impl Into<String>, owner_id: impl Into<String>) -> Self {
        Self {
            owner_type: owner_type.into(),
            owner_id: owner_id.into(),
        }
    }

    pub fn tenant(tenant_id: impl Into<String>) -> Self {
        Self::new("tenant", tenant_id)
    }

    pub fn organization(organization_id: impl Into<String>) -> Self {
        Self::new("organization", organization_id)
    }

    pub fn iam_user(user_id: impl Into<String>) -> Self {
        Self::new("user", user_id)
    }

    pub fn service(service_id: impl Into<String>) -> Self {
        Self::new("service", service_id)
    }

    pub fn device(device_id: impl Into<String>) -> Self {
        Self::new("device", device_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotRequestContext {
    pub tenant_id: String,
    pub organization_id: String,
    pub user_id: Option<String>,
    pub actor: AiotActorRef,
    pub data_scope: Vec<String>,
    pub permission_scope: Vec<String>,
    pub trace_id: Option<String>,
    pub deployment_mode: Option<String>,
}

impl AiotRequestContext {
    pub fn new(tenant_id: impl Into<String>, organization_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            organization_id: organization_id.into(),
            user_id: None,
            actor: AiotActorRef::system(),
            data_scope: Vec::new(),
            permission_scope: Vec::new(),
            trace_id: None,
            deployment_mode: None,
        }
    }

    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn with_actor(mut self, actor: AiotActorRef) -> Self {
        self.actor = actor;
        self
    }

    pub fn with_permission(mut self, permission: impl Into<String>) -> Self {
        self.permission_scope.push(permission.into());
        self
    }

    pub fn with_data_scope(mut self, scope: impl Into<String>) -> Self {
        self.data_scope.push(scope.into());
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permission_scope
            .iter()
            .any(|candidate| candidate == permission)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotComponentManifest {
    pub name: String,
    pub version: String,
    pub domain: String,
    pub capabilities: Vec<String>,
    pub required_features: Vec<String>,
    pub config_schema: Option<String>,
}

impl AiotComponentManifest {
    pub fn new(name: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            domain: domain.into(),
            capabilities: Vec::new(),
            required_features: Vec::new(),
            config_schema: None,
        }
    }

    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    pub fn with_required_feature(mut self, feature: impl Into<String>) -> Self {
        self.required_features.push(feature.into());
        self
    }

    pub fn with_config_schema(mut self, schema: impl Into<String>) -> Self {
        self.config_schema = Some(schema.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotDomainRecord {
    pub domain: &'static str,
    pub database_prefix: &'static str,
    pub api_tag: &'static str,
    pub sdk_namespace: &'static str,
    pub permission_prefix: &'static str,
    pub event_prefix: &'static str,
    pub capabilities: Vec<&'static str>,
    pub external_shared_kernels: Vec<&'static str>,
}

pub fn aiot_domain_record() -> AiotDomainRecord {
    AiotDomainRecord {
        domain: "iot",
        database_prefix: "iot",
        api_tag: "iot",
        sdk_namespace: "iot",
        permission_prefix: "iot",
        event_prefix: "iot",
        capabilities: vec![
            "productCatalog",
            "hardwareProfile",
            "protocolProfile",
            "deviceRegistry",
            "protocolGateway",
            "sessionRuntime",
            "capabilityModel",
            "commandControl",
            "deviceTwin",
            "telemetryEvent",
            "otaProvisioning",
            "edgeGateway",
            "operationsObservability",
        ],
        external_shared_kernels: vec!["sdkwork-appbase.iam"],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotApiSurface {
    pub name: &'static str,
    pub prefix: &'static str,
    pub sdk_package: &'static str,
    pub openapi_required: bool,
    pub generated_sdk_required: bool,
    pub operation_id_examples: Vec<&'static str>,
}

pub fn standard_api_surfaces() -> Vec<AiotApiSurface> {
    vec![
        AiotApiSurface {
            name: "app",
            prefix: IOT_APP_API_PREFIX,
            sdk_package: "@sdkwork/aiot-app-sdk",
            openapi_required: true,
            generated_sdk_required: true,
            operation_id_examples: vec![
                "devices.list",
                "devices.retrieve",
                "devices.commands.create",
                "devices.twin.retrieve",
                "devices.events.list",
            ],
        },
        AiotApiSurface {
            name: "backend",
            prefix: IOT_BACKEND_API_PREFIX,
            sdk_package: "@sdkwork/aiot-backend-sdk",
            openapi_required: true,
            generated_sdk_required: true,
            operation_id_examples: vec![
                "products.list",
                "hardwareProfiles.list",
                "protocolProfiles.list",
                "capabilityModels.retrieve",
                "devices.sessions.delete",
                "devices.commands.cancel",
                "devices.credentials.create",
                "firmwareRollouts.create",
                "protocolAdapters.list",
            ],
        },
    ]
}

pub fn standard_permissions() -> Vec<&'static str> {
    vec![
        IOT_PERMISSION_PRODUCTS_READ,
        IOT_PERMISSION_PRODUCTS_WRITE,
        IOT_PERMISSION_PROFILES_READ,
        IOT_PERMISSION_PROFILES_WRITE,
        IOT_PERMISSION_DEVICES_READ,
        IOT_PERMISSION_DEVICES_WRITE,
        IOT_PERMISSION_DEVICES_BIND,
        IOT_PERMISSION_DEVICES_DELETE,
        IOT_PERMISSION_SESSIONS_READ,
        IOT_PERMISSION_SESSIONS_DISCONNECT,
        IOT_PERMISSION_COMMANDS_READ,
        IOT_PERMISSION_COMMANDS_EXECUTE,
        IOT_PERMISSION_COMMANDS_CANCEL,
        IOT_PERMISSION_TWINS_READ,
        IOT_PERMISSION_TWINS_WRITE,
        IOT_PERMISSION_TELEMETRY_READ,
        IOT_PERMISSION_FIRMWARE_READ,
        IOT_PERMISSION_FIRMWARE_WRITE,
        IOT_PERMISSION_FIRMWARE_ROLLOUT,
        IOT_PERMISSION_PROTOCOL_ADAPTERS_READ,
        IOT_PERMISSION_PROTOCOL_ADAPTERS_WRITE,
        IOT_PERMISSION_RUNTIME_READ,
    ]
}

pub fn aiot_component_manifest() -> AiotComponentManifest {
    AiotComponentManifest::new("sdkwork-aiot-server", "iot")
        .with_capability("embedded_runtime")
        .with_capability("standalone_server")
        .with_capability("protocol_plugins")
        .with_capability("generated_sdk_contracts")
        .with_capability("device_registry")
        .with_capability("command_control")
        .with_required_feature("external_appbase_iam_context")
        .with_required_feature("openapi_sdkwork_v3")
        .with_required_feature("iot_prefixed_database_contracts")
        .with_config_schema("AiotConfig")
}
