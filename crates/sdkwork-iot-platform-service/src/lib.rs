#![allow(private_interfaces)]

mod api_response;
mod pagination;

pub use api_response::{
    aiot_wire_code_to_result, problem_detail_from_request, problem_detail_from_wire_code,
    problem_detail_response, resolve_trace_id,
};

use api_response::{
    domain_not_found_response, json_collection_response, standard_command_acceptance_response,
    standard_resource_response,
};
use pagination::{page_params_from_request, PageQuery};

fn require_page_params(request: &HttpRequest) -> Result<PageQuery, HttpResponse> {
    page_params_from_request(request)
        .map_err(|code| problem_detail_response(&resolve_trace_id(request), code, code.title()))
}

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::firmware_rollout_planner::{
    firmware_deployment_payload_json, resolve_rollout_target_device_ids, rollout_force_from_policy,
    RolloutTargetPolicyError,
};
use sdkwork_aiot_contract::{
    AiotRequestContext, IOT_PERMISSION_COMMANDS_CANCEL, IOT_PERMISSION_COMMANDS_EXECUTE,
    IOT_PERMISSION_COMMANDS_READ, IOT_PERMISSION_DEVICES_DELETE, IOT_PERMISSION_DEVICES_READ,
    IOT_PERMISSION_DEVICES_WRITE, IOT_PERMISSION_FIRMWARE_READ, IOT_PERMISSION_FIRMWARE_ROLLOUT,
    IOT_PERMISSION_FIRMWARE_WRITE, IOT_PERMISSION_PRODUCTS_READ, IOT_PERMISSION_PRODUCTS_WRITE,
    IOT_PERMISSION_PROFILES_READ, IOT_PERMISSION_PROFILES_WRITE,
    IOT_PERMISSION_PROTOCOL_ADAPTERS_READ, IOT_PERMISSION_RUNTIME_READ,
    IOT_PERMISSION_SESSIONS_DISCONNECT, IOT_PERMISSION_SESSIONS_READ,
    IOT_PERMISSION_TELEMETRY_READ, IOT_PERMISSION_TWINS_READ, IOT_PERMISSION_TWINS_WRITE,
};
use sdkwork_aiot_observability::emit_api_request_trace;
use sdkwork_aiot_protocol::{standard_protocol_catalog, CapabilityBridge, ProtocolPluginScope};
use sdkwork_aiot_service_host::{
    standard_aiot_runtime, AiotRuntime, RuntimeBuildError, RuntimeMode,
};
use sdkwork_aiot_storage::{
    paginate_bounded_catalog, paginate_vec, AiotCommandCreateCommand, AiotCommandRecord,
    AiotCommandRepository, AiotCommandRepositoryError, AiotDeviceCreateCommand,
    AiotDeviceEventRecord, AiotDeviceRecord, AiotDeviceRepository, AiotDeviceRepositoryError,
    AiotDeviceSessionRecord, AiotDeviceSessionRepository, AiotDeviceTwinRepository,
    AiotDeviceTwinRepositoryError, AiotDeviceTwinSnapshot, AiotDeviceUpdateCommand,
    AiotEventRepository, AiotEventRepositoryError, AiotOffsetListResult, AiotStorageAssociation,
    AiotTwinPropertyUpsertCommand, InMemoryAiotCommandRepository, InMemoryAiotDeviceRepository,
    InMemoryAiotDeviceSessionRepository, InMemoryAiotDeviceTwinRepository,
    InMemoryAiotEventRepository, OffsetListPageParams,
};
use sdkwork_aiot_transport::{build_health_response, HttpRequest, HttpResponse, HttpStatus};
use sdkwork_iot_device_service::{CapabilityDefinition, CapabilityKind, ProtocolProfile};
use sdkwork_utils_rust::{base64url_decode, hex_encode, hmac_sha256, secure_compare};

mod firmware_rollout_planner;
mod service_stores;
mod sqlite_admin;

use serde_json::{Map as JsonMap, Value as JsonValue};
pub use service_stores::{
    configured_device_db_path, open_admin_service_stores, open_app_service_stores,
    AiotAdminServiceStores, AiotAppServiceStores,
};
pub use sqlite_admin::{AiotCatalogRepositoryHandle, AiotFirmwareRepositoryHandle};

const AUTH_FAILURE_RATE_LIMIT_PER_MINUTE: u32 = 100;
const AUTH_FAILURE_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

static AUTH_FAILURE_RATE_LIMITER: Mutex<BTreeMap<String, AuthFailureWindow>> =
    Mutex::new(BTreeMap::new());

#[derive(Debug, Clone)]
struct AuthFailureWindow {
    window_start: Instant,
    count: u32,
}

pub trait AiotIamContextResolver: Send + Sync {
    fn resolve(&self, request: &HttpRequest) -> Result<AiotRequestContext, HttpResponse>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultSdkworkIamContextResolver;

impl AiotIamContextResolver for DefaultSdkworkIamContextResolver {
    fn resolve(&self, request: &HttpRequest) -> Result<AiotRequestContext, HttpResponse> {
        if is_blank_header(request, "authorization")
            || (access_token_header(request).is_none()
                && is_blank_header(request, "access-token")
                && is_blank_header(request, "sdkwork-access-token"))
        {
            if auth_failure_rate_limited(request) {
                return Err(problem_response(
                    HttpStatus::TooManyRequests,
                    "api.auth.rate_limited",
                    "Too Many Requests",
                ));
            }
            return Err(problem_response(
                HttpStatus::Unauthorized,
                "api.auth.missing_dual_token",
                "SDKWork dual token is required",
            ));
        }

        if trust_proxy_headers_enabled() && has_sdkwork_proxy_context_headers(request) {
            resolve_protected_context_from_proxy_headers(request)
        } else {
            resolve_protected_context_from_token(request)
        }
    }
}

static DEFAULT_IAM_CONTEXT_RESOLVER: DefaultSdkworkIamContextResolver =
    DefaultSdkworkIamContextResolver;

pub fn default_iam_context_resolver() -> &'static DefaultSdkworkIamContextResolver {
    &DEFAULT_IAM_CONTEXT_RESOLVER
}

pub trait AiotCredentialRepository: Send + Sync {
    fn create_credential(
        &self,
        association: AiotStorageAssociation,
        command: AiotCredentialCreateCommand,
    ) -> Result<AiotDeviceCredentialRecord, HttpResponse>;

    fn list_credentials(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceCredentialRecord>, HttpResponse>;

    fn get_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Option<AiotDeviceCredentialRecord>;

    fn delete_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Result<(), HttpResponse>;
}

pub struct SqliteCredentialRepositoryAdapter {
    inner: Arc<sdkwork_aiot_storage_sqlx::SqliteSqlxCredentialRepository>,
}

impl SqliteCredentialRepositoryAdapter {
    pub fn new_in_memory() -> Result<Self, String> {
        sdkwork_aiot_storage_sqlx::SqliteSqlxCredentialRepository::new_in_memory()
            .map(|inner| Self {
                inner: Arc::new(inner),
            })
            .map_err(|error| error.to_string())
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        sdkwork_aiot_storage_sqlx::SqliteSqlxCredentialRepository::open(path)
            .map(Self::from_repository)
            .map_err(|error| error.to_string())
    }

    pub fn from_repository(
        inner: sdkwork_aiot_storage_sqlx::SqliteSqlxCredentialRepository,
    ) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn verify_bearer_token(&self, device_id: &str, token: &str) -> bool {
        self.inner.verify_bearer_token(device_id, token)
    }
}

impl AiotCredentialRepository for SqliteCredentialRepositoryAdapter {
    fn create_credential(
        &self,
        association: AiotStorageAssociation,
        command: AiotCredentialCreateCommand,
    ) -> Result<AiotDeviceCredentialRecord, HttpResponse> {
        self.inner
            .create_credential(sdkwork_aiot_storage_sqlx::SqliteCredentialCreateCommand {
                association,
                device_id: command.device_id,
                credential_type: command.credential_type,
                expires_at: command.expires_at,
            })
            .map(|record| AiotDeviceCredentialRecord {
                credential_id: record.credential_id,
                tenant_id: record.tenant_id,
                organization_id: record.organization_id,
                device_id: record.device_id,
                credential_type: record.credential_type,
                status: record.status,
                expires_at: record.expires_at,
                created_at: record.created_at,
                revoked_at: record.revoked_at,
                issued_secret: record.issued_secret,
            })
            .map_err(|_| {
                problem_response(
                    HttpStatus::InternalServerError,
                    "api.storage.write_failed",
                    "Storage write failed",
                )
            })
    }

    fn list_credentials(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceCredentialRecord>, HttpResponse> {
        let page = self
            .inner
            .list_credentials(association, device_id, params)
            .map_err(|_| {
                problem_response(
                    HttpStatus::InternalServerError,
                    "api.storage.read_failed",
                    "Storage read failed",
                )
            })?;
        Ok(AiotOffsetListResult {
            items: page
                .items
                .into_iter()
                .map(|record| AiotDeviceCredentialRecord {
                    credential_id: record.credential_id,
                    tenant_id: record.tenant_id,
                    organization_id: record.organization_id,
                    device_id: record.device_id,
                    credential_type: record.credential_type,
                    status: record.status,
                    expires_at: record.expires_at,
                    created_at: record.created_at,
                    revoked_at: record.revoked_at,
                    issued_secret: None,
                })
                .collect(),
            total: page.total,
        })
    }

    fn get_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Option<AiotDeviceCredentialRecord> {
        self.inner
            .get_credential(association, device_id, credential_id)
            .map(|record| AiotDeviceCredentialRecord {
                credential_id: record.credential_id,
                tenant_id: record.tenant_id,
                organization_id: record.organization_id,
                device_id: record.device_id,
                credential_type: record.credential_type,
                status: record.status,
                expires_at: record.expires_at,
                created_at: record.created_at,
                revoked_at: record.revoked_at,
                issued_secret: None,
            })
    }

    fn delete_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Result<(), HttpResponse> {
        self.inner
            .revoke_credential(association, device_id, credential_id)
            .map_err(|error| match error {
                sdkwork_aiot_storage_sqlx::SqliteCredentialRepositoryError::CredentialNotFound => {
                    credential_not_found_response(credential_id)
                }
                sdkwork_aiot_storage_sqlx::SqliteCredentialRepositoryError::PersistenceFailure => {
                    problem_response(
                        HttpStatus::InternalServerError,
                        "api.storage.write_failed",
                        "Storage write failed",
                    )
                }
            })
    }
}

pub fn dev_mode_enabled() -> bool {
    std::env::var("SDKWORK_AIOT_DEV_MODE").as_deref() == Ok("1")
}

const PRODUCTION_MIN_SECRET_LENGTH: usize = 32;

/// Refuses to start when production environment enables dev-mode auth bypass, weak secrets,
/// or ephemeral in-memory device persistence.
pub fn assert_production_environment_safety() {
    if std::env::var("SDKWORK_AIOT_ENVIRONMENT").as_deref() != Ok("production") {
        return;
    }

    if dev_mode_enabled() {
        eprintln!(
            "FATAL: SDKWORK_AIOT_DEV_MODE=1 is forbidden when SDKWORK_AIOT_ENVIRONMENT=production"
        );
        std::process::exit(1);
    }

    if !sdkwork_aiot_storage_sqlx::device_database_config_is_durable_from_env() {
        eprintln!(
            "FATAL: production requires durable device persistence via SDKWORK_AIOT_DEVICE_DB_PATH or SDKWORK_AIOT_DEVICE_DATABASE_* env keys"
        );
        std::process::exit(1);
    }

    let pepper = std::env::var("SDKWORK_AIOT_CREDENTIAL_PEPPER").unwrap_or_default();
    if pepper.trim().len() < PRODUCTION_MIN_SECRET_LENGTH {
        eprintln!(
            "FATAL: SDKWORK_AIOT_CREDENTIAL_PEPPER must be at least {PRODUCTION_MIN_SECRET_LENGTH} characters when SDKWORK_AIOT_ENVIRONMENT=production"
        );
        std::process::exit(1);
    }

    let internal_token = std::env::var("SDKWORK_AIOT_INTERNAL_TOKEN").unwrap_or_default();
    if internal_token.trim().len() < PRODUCTION_MIN_SECRET_LENGTH {
        eprintln!(
            "FATAL: SDKWORK_AIOT_INTERNAL_TOKEN must be at least {PRODUCTION_MIN_SECRET_LENGTH} characters when SDKWORK_AIOT_ENVIRONMENT=production"
        );
        std::process::exit(1);
    }

    if trust_proxy_headers_enabled() {
        eprintln!(
            "INFO: production proxy-header trust requires x-sdkwork-proxy-auth matching SDKWORK_AIOT_INTERNAL_TOKEN on every request with X-Sdkwork context headers"
        );
    }
}

fn trust_proxy_headers_enabled() -> bool {
    std::env::var("SDKWORK_AIOT_TRUST_PROXY_HEADERS").as_deref() == Ok("1")
}

fn production_environment_enabled() -> bool {
    std::env::var("SDKWORK_AIOT_ENVIRONMENT").as_deref() == Ok("production")
}

fn proxy_header_context_authorized(request: &HttpRequest) -> Result<(), HttpResponse> {
    if !production_environment_enabled() {
        return Ok(());
    }

    let expected = std::env::var("SDKWORK_AIOT_INTERNAL_TOKEN")
        .unwrap_or_default()
        .trim()
        .to_string();
    if expected.len() < PRODUCTION_MIN_SECRET_LENGTH {
        return Err(problem_response(
            HttpStatus::Forbidden,
            "api.auth.missing_dual_token",
            "Proxy context requires configured internal token in production",
        ));
    }

    let provided = request
        .header("x-sdkwork-proxy-auth")
        .or_else(|| request.header("x-sdkwork-internal-proxy-auth"))
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if provided.is_some_and(|value| secure_compare(value, expected.as_str())) {
        return Ok(());
    }

    Err(problem_response(
        HttpStatus::Unauthorized,
        "api.auth.missing_dual_token",
        "Trusted proxy authentication is required for context headers in production",
    ))
}

fn resolve_protected_context_from_proxy_headers(
    request: &HttpRequest,
) -> Result<AiotRequestContext, HttpResponse> {
    if let Err(response) = proxy_header_context_authorized(request) {
        return Err(response);
    }

    let tenant_id = required_header(request, "x-sdkwork-tenant-id").map_err(|_| {
        problem_response(
            HttpStatus::Forbidden,
            "api.context.missing",
            "Resolved appbase context is required",
        )
    })?;
    let organization_id = required_header(request, "x-sdkwork-organization-id").map_err(|_| {
        problem_response(
            HttpStatus::Forbidden,
            "api.context.missing",
            "Resolved appbase context is required",
        )
    })?;

    parse_i64(tenant_id).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.context.invalid_tenant_id",
            "Resolved tenant id is invalid",
        )
    })?;
    parse_i64(organization_id).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.context.invalid_organization_id",
            "Resolved organization id is invalid",
        )
    })?;

    let mut ctx = AiotRequestContext::new(tenant_id, organization_id);

    if let Some(user_id) = optional_header(request, "x-sdkwork-user-id") {
        parse_i64(user_id).map_err(|_| {
            problem_response(
                HttpStatus::BadRequest,
                "api.context.invalid_user_id",
                "Resolved user id is invalid",
            )
        })?;
        ctx = ctx.with_user(user_id);
    }

    if let Some(data_scope) = optional_header(request, "x-sdkwork-data-scope") {
        data_scope.parse::<i32>().map_err(|_| {
            problem_response(
                HttpStatus::BadRequest,
                "api.context.invalid_data_scope",
                "Resolved data scope is invalid",
            )
        })?;
        ctx = ctx.with_data_scope(data_scope);
    }

    for permission in permission_scope_headers(request) {
        ctx = ctx.with_permission(permission);
    }

    Ok(ctx)
}

fn resolve_protected_context_from_token(
    request: &HttpRequest,
) -> Result<AiotRequestContext, HttpResponse> {
    let (auth_claims, access_claims) = resolve_dual_token_claims(request)?;

    let tenant_id = json_claim_string(&access_claims, &["tenant_id", "tenantId"])
        .or_else(|| json_claim_string(&auth_claims, &["tenant_id", "tenantId"]))
        .ok_or_else(|| {
            problem_response(
                HttpStatus::Forbidden,
                "api.context.missing",
                "Resolved appbase context is required",
            )
        })?;
    let organization_id = json_claim_string(&access_claims, &["organization_id", "organizationId"])
        .or_else(|| json_claim_string(&auth_claims, &["organization_id", "organizationId"]))
        .ok_or_else(|| {
            problem_response(
                HttpStatus::Forbidden,
                "api.context.missing",
                "Resolved appbase context is required",
            )
        })?;

    parse_i64(&tenant_id).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.context.invalid_tenant_id",
            "Resolved tenant id is invalid",
        )
    })?;
    parse_i64(&organization_id).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.context.invalid_organization_id",
            "Resolved organization id is invalid",
        )
    })?;

    let mut ctx = AiotRequestContext::new(tenant_id, organization_id);
    if let Some(user_id) = json_claim_string(&auth_claims, &["sub", "user_id", "userId"])
        .or_else(|| json_claim_string(&access_claims, &["sub", "user_id", "userId"]))
    {
        parse_i64(&user_id).map_err(|_| {
            problem_response(
                HttpStatus::BadRequest,
                "api.context.invalid_user_id",
                "Resolved user id is invalid",
            )
        })?;
        ctx = ctx.with_user(user_id);
    }

    if let Some(data_scope) = json_claim_string(&access_claims, &["data_scope", "dataScope"])
        .or_else(|| json_claim_string(&auth_claims, &["data_scope", "dataScope"]))
    {
        ctx = ctx.with_data_scope(data_scope);
    }

    for permission in jwt_permissions_from_claims(&access_claims) {
        ctx = ctx.with_permission(permission);
    }
    for permission in jwt_permissions_from_claims(&auth_claims) {
        ctx = ctx.with_permission(permission);
    }
    for permission in dev_permissions_from_env() {
        ctx = ctx.with_permission(permission);
    }

    Ok(ctx)
}

fn resolve_dual_token_claims(
    request: &HttpRequest,
) -> Result<(JsonValue, JsonValue), HttpResponse> {
    if let Some((auth_claims, access_claims)) = dev_fixture_dual_token_claims(request) {
        return Ok((auth_claims, access_claims));
    }

    let auth_token = bearer_token_from_request(request).ok_or_else(|| {
        problem_response(
            HttpStatus::Unauthorized,
            "api.auth.invalid_bearer",
            "Bearer token is invalid",
        )
    })?;
    let auth_claims = parse_bearer_jwt_claims(&auth_token).map_err(|code| {
        problem_response(HttpStatus::Unauthorized, code, "Bearer token is invalid")
    })?;

    let access_token = access_token_header(request)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            problem_response(
                HttpStatus::Unauthorized,
                "api.auth.missing_dual_token",
                "SDKWork dual token is required",
            )
        })?;
    let access_claims = if access_token.contains(' ') {
        parse_bearer_jwt_claims(access_token).map_err(|code| {
            problem_response(HttpStatus::Unauthorized, code, "Bearer token is invalid")
        })?
    } else {
        parse_bearer_jwt_claims(access_token).map_err(|code| {
            problem_response(HttpStatus::Unauthorized, code, "Bearer token is invalid")
        })?
    };

    Ok((auth_claims, access_claims))
}

fn has_sdkwork_proxy_context_headers(request: &HttpRequest) -> bool {
    !is_blank_header(request, "x-sdkwork-tenant-id")
        || !is_blank_header(request, "x-sdkwork-organization-id")
        || !is_blank_header(request, "x-sdkwork-user-id")
        || !is_blank_header(request, "x-sdkwork-data-scope")
        || !is_blank_header(request, "x-sdkwork-permission-scope")
}

fn dev_fixture_dual_token_claims(request: &HttpRequest) -> Option<(JsonValue, JsonValue)> {
    if !dev_mode_enabled() {
        return None;
    }

    let auth_token = bearer_token_from_request(request)?;
    let access_token = access_token_header(request)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if auth_token != "app-token" || access_token != "user-token" {
        if auth_token == "app-token-missing-context" && access_token == "user-token-missing-context"
        {
            return Some((
                serde_json::json!({
                    "sub": "30001",
                    "user_id": "30001",
                }),
                serde_json::json!({}),
            ));
        }
        return None;
    }

    Some((
        serde_json::json!({
            "sub": "30001",
            "user_id": "30001",
        }),
        serde_json::json!({
            "tenant_id": "100001",
            "organization_id": "0",
        }),
    ))
}

fn jwt_permissions_from_claims(claims: &JsonValue) -> Vec<String> {
    let mut permissions = Vec::new();
    for key in [
        "permission_scope",
        "permissionScope",
        "permissions",
        "scope",
    ] {
        let Some(value) = claims.get(key) else {
            continue;
        };
        match value {
            JsonValue::String(text) => {
                permissions.extend(
                    text.split(',')
                        .map(str::trim)
                        .filter(|entry| !entry.is_empty())
                        .map(str::to_string),
                );
            }
            JsonValue::Array(items) => {
                for item in items {
                    if let JsonValue::String(text) = item {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            permissions.push(trimmed.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    permissions.sort_unstable();
    permissions.dedup();
    permissions
}

fn bearer_token_from_request(request: &HttpRequest) -> Option<String> {
    optional_header(request, "authorization")
        .and_then(extract_bearer_token)
        .or_else(|| {
            access_token_header(request).and_then(|value| {
                if value.contains(' ') {
                    extract_bearer_token(value)
                } else {
                    Some(value.to_string())
                }
            })
        })
}

fn extract_bearer_token(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let token = trimmed
        .strip_prefix("Bearer ")
        .or_else(|| trimmed.strip_prefix("bearer "))?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn parse_bearer_jwt_claims(token: &str) -> Result<JsonValue, &'static str> {
    if let Some(secret) = dev_auth_secret() {
        if !verify_dev_hmac_token(token, &secret) {
            return Err("api.auth.invalid_bearer");
        }
    } else if !dev_mode_enabled() {
        return Err("api.auth.invalid_bearer");
    }

    let parts = token.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err("api.auth.invalid_bearer");
    }

    let payload = base64url_decode(parts[1]).ok_or("api.auth.invalid_bearer")?;
    serde_json::from_slice(&payload).map_err(|_| "api.auth.invalid_bearer")
}

fn json_claim_string(claims: &JsonValue, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        claims.get(*key).and_then(|value| match value {
            JsonValue::String(text) if !text.trim().is_empty() => Some(text.clone()),
            JsonValue::Number(number) => Some(number.to_string()),
            _ => None,
        })
    })
}

fn dev_auth_secret() -> Option<String> {
    std::env::var("SDKWORK_AIOT_DEV_AUTH_SECRET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn dev_permissions_from_env() -> Vec<String> {
    if !dev_mode_enabled() {
        return Vec::new();
    }

    std::env::var("SDKWORK_AIOT_DEV_PERMISSIONS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn verify_dev_hmac_token(token: &str, secret: &str) -> bool {
    let parts = token.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return false;
    }
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let Some(signature) = base64url_decode(parts[2]) else {
        return false;
    };
    let expected_hex = hmac_sha256(signing_input.as_bytes(), secret.as_bytes());
    let signature_hex = hex_encode(&signature);
    secure_compare(&signature_hex, &expected_hex)
}

fn auth_failure_rate_limited(request: &HttpRequest) -> bool {
    let client_ip = client_ip_from_request(request);
    let mut guard = AUTH_FAILURE_RATE_LIMITER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let now = Instant::now();
    let entry = guard.entry(client_ip).or_insert(AuthFailureWindow {
        window_start: now,
        count: 0,
    });
    if now.duration_since(entry.window_start) >= AUTH_FAILURE_RATE_LIMIT_WINDOW {
        entry.window_start = now;
        entry.count = 0;
    }
    entry.count = entry.count.saturating_add(1);
    entry.count > AUTH_FAILURE_RATE_LIMIT_PER_MINUTE
}

fn client_ip_from_request(request: &HttpRequest) -> String {
    optional_header(request, "x-forwarded-for")
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| optional_header(request, "x-real-ip"))
        .unwrap_or("unknown")
        .to_string()
}

fn apply_security_headers(mut response: HttpResponse) -> HttpResponse {
    response = response
        .with_header("x-content-type-options", "nosniff")
        .with_header("x-frame-options", "DENY")
        .with_header("referrer-policy", "no-referrer")
        .with_header("cache-control", "no-store")
        .with_header(
            "content-security-policy",
            "default-src 'none'; frame-ancestors 'none'",
        );
    response
}

fn cors_allowed_origins() -> Vec<String> {
    std::env::var("SDKWORK_AIOT_CORS_ALLOWED_ORIGINS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn cors_allowed_origin(request: &HttpRequest) -> Option<String> {
    let origin = optional_header(request, "origin")?;
    let allowed = cors_allowed_origins();
    let environment = std::env::var("SDKWORK_AIOT_ENVIRONMENT")
        .unwrap_or_else(|_| "development".to_owned())
        .trim()
        .to_ascii_lowercase();
    let development = matches!(
        environment.as_str(),
        "development" | "dev" | "local" | "test" | "testing"
    );
    if allowed.iter().any(|candidate| candidate == origin)
        || (development && sdkwork_web_core::is_development_private_network_origin(origin))
    {
        Some(origin.to_string())
    } else {
        None
    }
}

fn apply_cors_headers(request: &HttpRequest, mut response: HttpResponse) -> HttpResponse {
    let Some(origin) = cors_allowed_origin(request) else {
        return response;
    };
    response = response
        .with_header("access-control-allow-origin", origin)
        .with_header("vary", "Origin")
        .with_header(
            "access-control-allow-headers",
            "Authorization, Access-Token, Content-Type, Idempotency-Key",
        )
        .with_header(
            "access-control-allow-methods",
            "GET, POST, PUT, PATCH, DELETE, OPTIONS",
        )
        .with_header("access-control-max-age", "600");
    response
}

fn cors_preflight_response(request: &HttpRequest) -> Option<HttpResponse> {
    if request.method != "OPTIONS" {
        return None;
    }
    let origin = cors_allowed_origin(request)?;
    Some(apply_security_headers(
        HttpResponse::new(HttpStatus::NoContent)
            .with_header("access-control-allow-origin", origin)
            .with_header("vary", "Origin")
            .with_header(
                "access-control-allow-headers",
                optional_header(request, "access-control-request-headers")
                    .unwrap_or("Authorization, Access-Token, Content-Type, Idempotency-Key"),
            )
            .with_header(
                "access-control-allow-methods",
                optional_header(request, "access-control-request-method")
                    .unwrap_or("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
            )
            .with_header("access-control-max-age", "600"),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiotApiSurface {
    Admin,
    App,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiotApiRouteContract {
    pub surface: AiotApiSurface,
    pub method: &'static str,
    pub path: &'static str,
    pub operation_id: &'static str,
    pub required_permission: &'static str,
}

pub fn standard_api_route_contracts() -> Vec<AiotApiRouteContract> {
    vec![
        AiotApiRouteContract {
            surface: AiotApiSurface::App,
            method: "GET",
            path: "/app/v3/api/iot/devices",
            operation_id: "devices.list",
            required_permission: IOT_PERMISSION_DEVICES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::App,
            method: "GET",
            path: "/app/v3/api/iot/devices/{deviceId}",
            operation_id: "devices.retrieve",
            required_permission: IOT_PERMISSION_DEVICES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::App,
            method: "POST",
            path: "/app/v3/api/iot/devices/{deviceId}/commands",
            operation_id: "devices.commands.create",
            required_permission: IOT_PERMISSION_COMMANDS_EXECUTE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::App,
            method: "GET",
            path: "/app/v3/api/iot/devices/{deviceId}/commands/{commandId}",
            operation_id: "devices.commands.retrieve",
            required_permission: IOT_PERMISSION_COMMANDS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::App,
            method: "GET",
            path: "/app/v3/api/iot/devices/{deviceId}/twin",
            operation_id: "devices.twin.retrieve",
            required_permission: IOT_PERMISSION_TWINS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::App,
            method: "GET",
            path: "/app/v3/api/iot/devices/{deviceId}/events",
            operation_id: "devices.events.list",
            required_permission: IOT_PERMISSION_DEVICES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/products",
            operation_id: "products.list",
            required_permission: IOT_PERMISSION_PRODUCTS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/products",
            operation_id: "products.create",
            required_permission: IOT_PERMISSION_PRODUCTS_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/products/{productId}",
            operation_id: "products.retrieve",
            required_permission: IOT_PERMISSION_PRODUCTS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "PUT",
            path: "/backend/v3/api/iot/products/{productId}",
            operation_id: "products.update",
            required_permission: IOT_PERMISSION_PRODUCTS_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/products/{productId}",
            operation_id: "products.delete",
            required_permission: IOT_PERMISSION_PRODUCTS_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/hardware_profiles",
            operation_id: "hardwareProfiles.list",
            required_permission: IOT_PERMISSION_PROFILES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/hardware_profiles",
            operation_id: "hardwareProfiles.create",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/hardware_profiles/{hardwareProfileId}",
            operation_id: "hardwareProfiles.retrieve",
            required_permission: IOT_PERMISSION_PROFILES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "PUT",
            path: "/backend/v3/api/iot/hardware_profiles/{hardwareProfileId}",
            operation_id: "hardwareProfiles.update",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/hardware_profiles/{hardwareProfileId}",
            operation_id: "hardwareProfiles.delete",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/protocol_profiles",
            operation_id: "protocolProfiles.list",
            required_permission: IOT_PERMISSION_PROFILES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/protocol_profiles",
            operation_id: "protocolProfiles.create",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/protocol_profiles/{protocolProfileId}",
            operation_id: "protocolProfiles.retrieve",
            required_permission: IOT_PERMISSION_PROFILES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "PUT",
            path: "/backend/v3/api/iot/protocol_profiles/{protocolProfileId}",
            operation_id: "protocolProfiles.update",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/protocol_profiles/{protocolProfileId}",
            operation_id: "protocolProfiles.delete",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/capability_models",
            operation_id: "capabilityModels.list",
            required_permission: IOT_PERMISSION_PROFILES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/capability_models",
            operation_id: "capabilityModels.create",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/capability_models/{capabilityModelId}",
            operation_id: "capabilityModels.retrieve",
            required_permission: IOT_PERMISSION_PROFILES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "PUT",
            path: "/backend/v3/api/iot/capability_models/{capabilityModelId}",
            operation_id: "capabilityModels.update",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/capability_models/{capabilityModelId}",
            operation_id: "capabilityModels.delete",
            required_permission: IOT_PERMISSION_PROFILES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices",
            operation_id: "devices.list",
            required_permission: IOT_PERMISSION_DEVICES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/devices",
            operation_id: "devices.create",
            required_permission: IOT_PERMISSION_DEVICES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices/{deviceId}",
            operation_id: "devices.retrieve",
            required_permission: IOT_PERMISSION_DEVICES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "PUT",
            path: "/backend/v3/api/iot/devices/{deviceId}",
            operation_id: "devices.update",
            required_permission: IOT_PERMISSION_DEVICES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/devices/{deviceId}",
            operation_id: "devices.delete",
            required_permission: IOT_PERMISSION_DEVICES_DELETE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices/{deviceId}/credentials",
            operation_id: "devices.credentials.list",
            required_permission: IOT_PERMISSION_DEVICES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices/{deviceId}/credentials/{credentialId}",
            operation_id: "devices.credentials.retrieve",
            required_permission: IOT_PERMISSION_DEVICES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/devices/{deviceId}/credentials",
            operation_id: "devices.credentials.create",
            required_permission: IOT_PERMISSION_DEVICES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/devices/{deviceId}/credentials/{credentialId}",
            operation_id: "devices.credentials.delete",
            required_permission: IOT_PERMISSION_DEVICES_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices/{deviceId}/sessions",
            operation_id: "devices.sessions.list",
            required_permission: IOT_PERMISSION_SESSIONS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/devices/{deviceId}/sessions/{sessionId}",
            operation_id: "devices.sessions.disconnect",
            required_permission: IOT_PERMISSION_SESSIONS_DISCONNECT,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices/{deviceId}/capabilities",
            operation_id: "devices.capabilities.list",
            required_permission: IOT_PERMISSION_DEVICES_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices/{deviceId}/commands",
            operation_id: "devices.commands.list",
            required_permission: IOT_PERMISSION_COMMANDS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices/{deviceId}/commands/{commandId}",
            operation_id: "devices.commands.retrieve",
            required_permission: IOT_PERMISSION_COMMANDS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/devices/{deviceId}/commands/{commandId}/cancel",
            operation_id: "devices.commands.cancel",
            required_permission: IOT_PERMISSION_COMMANDS_CANCEL,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/devices/{deviceId}/twin",
            operation_id: "devices.twin.retrieve",
            required_permission: IOT_PERMISSION_TWINS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "PATCH",
            path: "/backend/v3/api/iot/devices/{deviceId}/twin",
            operation_id: "devices.twin.update",
            required_permission: IOT_PERMISSION_TWINS_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/firmware_artifacts",
            operation_id: "firmwareArtifacts.list",
            required_permission: IOT_PERMISSION_FIRMWARE_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/firmware_artifacts",
            operation_id: "firmwareArtifacts.create",
            required_permission: IOT_PERMISSION_FIRMWARE_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/firmware_artifacts/{artifactId}",
            operation_id: "firmwareArtifacts.retrieve",
            required_permission: IOT_PERMISSION_FIRMWARE_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "PUT",
            path: "/backend/v3/api/iot/firmware_artifacts/{artifactId}",
            operation_id: "firmwareArtifacts.update",
            required_permission: IOT_PERMISSION_FIRMWARE_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/firmware_artifacts/{artifactId}",
            operation_id: "firmwareArtifacts.delete",
            required_permission: IOT_PERMISSION_FIRMWARE_WRITE,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/firmware_rollouts",
            operation_id: "firmwareRollouts.list",
            required_permission: IOT_PERMISSION_FIRMWARE_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "POST",
            path: "/backend/v3/api/iot/firmware_rollouts",
            operation_id: "firmwareRollouts.create",
            required_permission: IOT_PERMISSION_FIRMWARE_ROLLOUT,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/firmware_rollouts/{rolloutId}",
            operation_id: "firmwareRollouts.retrieve",
            required_permission: IOT_PERMISSION_FIRMWARE_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "PUT",
            path: "/backend/v3/api/iot/firmware_rollouts/{rolloutId}",
            operation_id: "firmwareRollouts.update",
            required_permission: IOT_PERMISSION_FIRMWARE_ROLLOUT,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "DELETE",
            path: "/backend/v3/api/iot/firmware_rollouts/{rolloutId}",
            operation_id: "firmwareRollouts.delete",
            required_permission: IOT_PERMISSION_FIRMWARE_ROLLOUT,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/events",
            operation_id: "events.list",
            required_permission: IOT_PERMISSION_TELEMETRY_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/protocol_adapters",
            operation_id: "protocolAdapters.list",
            required_permission: IOT_PERMISSION_PROTOCOL_ADAPTERS_READ,
        },
        AiotApiRouteContract {
            surface: AiotApiSurface::Admin,
            method: "GET",
            path: "/backend/v3/api/iot/runtime/capacity",
            operation_id: "runtime.capacity.retrieve",
            required_permission: IOT_PERMISSION_RUNTIME_READ,
        },
    ]
}

fn auth_critical_rate_limit_tier(operation_id: &str) -> Option<&'static str> {
    match operation_id {
        "devices.commands.create"
        | "devices.create"
        | "devices.delete"
        | "devices.credentials.create"
        | "devices.credentials.delete"
        | "devices.commands.cancel"
        | "firmwareRollouts.create" => Some("authCritical"),
        _ => None,
    }
}

pub fn standard_route_manifest_document(surface: AiotApiSurface) -> serde_json::Value {
    let (package_name, api_surface, prefix, api_authority, sdk_family, crate_root) = match surface {
        AiotApiSurface::App => (
            "sdkwork-aiot-app-api",
            "app-api",
            "/app/v3/api",
            "sdkwork-aiot-app-api",
            "sdkwork-aiot-app-sdk",
            "services/sdkwork-aiot-app-api",
        ),
        AiotApiSurface::Admin => (
            "sdkwork-aiot-admin-api",
            "backend-api",
            "/backend/v3/api",
            "sdkwork-aiot-backend-api",
            "sdkwork-aiot-backend-sdk",
            "services/sdkwork-aiot-admin-api",
        ),
    };

    let routes = standard_api_route_contracts()
        .into_iter()
        .filter(|route| route.surface == surface)
        .map(|route| {
            let primary_tag = route
                .operation_id
                .split('.')
                .next()
                .unwrap_or("iot")
                .to_string();
            let mut route_entry = serde_json::json!({
                "method": route.method,
                "path": route.path,
                "operationId": route.operation_id,
                "requestContext": "WebRequestContext",
                "apiSurface": api_surface,
                "tags": [format!("iot.{primary_tag}")],
                "auth": {
                    "mode": "dual-token",
                    "required": true,
                    "permission": route.required_permission,
                    "tenantScope": "tenant",
                    "dataScope": "organization"
                },
                "handler": {
                    "module": "sdkwork_iot_platform_service",
                    "name": route.operation_id
                },
                "schemas": {
                    "request": null,
                    "response": null,
                    "problem": "ProblemDetail"
                },
                "ownership": {
                    "owner": "sdkwork-aiot",
                    "apiAuthority": api_authority
                },
                "source": {
                    "file": "crates/sdkwork-iot-platform-service/src/lib.rs"
                }
            });
            if let Some(tier) = auth_critical_rate_limit_tier(route.operation_id) {
                route_entry["rateLimitTier"] = serde_json::Value::String(tier.to_string());
            }
            route_entry
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "schemaVersion": 1,
        "kind": "sdkwork.route.manifest",
        "packageName": package_name,
        "surface": api_surface,
        "owner": "sdkwork-aiot",
        "domain": "iot",
        "capability": "iot",
        "apiAuthority": api_authority,
        "sdkFamily": sdk_family,
        "prefix": prefix,
        "source": {
            "crateRoot": crate_root,
            "crateImport": package_name.replace('-', "_")
        },
        "routes": routes
    })
}

pub fn standard_route_manifest_json(surface: AiotApiSurface) -> String {
    serde_json::to_string_pretty(&standard_route_manifest_document(surface))
        .expect("serialize route manifest")
}

pub fn route_contract_for_request(
    surface: AiotApiSurface,
    request: &HttpRequest,
) -> Option<AiotApiRouteContract> {
    standard_api_route_contracts().into_iter().find(|route| {
        route.surface == surface
            && route.method.eq_ignore_ascii_case(&request.method)
            && route_path_matches(route.path, &request.path)
    })
}

#[derive(Clone)]
pub struct AiotApiServer {
    surface: AiotApiSurface,
    runtime: AiotRuntime,
    device_repository: Arc<dyn AiotDeviceRepository>,
    command_repository: Arc<dyn AiotCommandRepository>,
    event_repository: Arc<dyn AiotEventRepository>,
    twin_repository: Arc<dyn AiotDeviceTwinRepository>,
    device_session_repository: Arc<dyn AiotDeviceSessionRepository>,
    credential_repository: Arc<dyn AiotCredentialRepository>,
    firmware_repository: Arc<AiotFirmwareRepositoryHandle>,
    catalog_repository: Arc<AiotCatalogRepositoryHandle>,
}

impl AiotApiServer {
    pub fn new(surface: AiotApiSurface, runtime: AiotRuntime) -> Self {
        Self {
            surface,
            runtime,
            device_repository: Arc::new(InMemoryAiotDeviceRepository::new()),
            command_repository: Arc::new(InMemoryAiotCommandRepository::new()),
            event_repository: Arc::new(InMemoryAiotEventRepository::new()),
            twin_repository: Arc::new(InMemoryAiotDeviceTwinRepository::new()),
            device_session_repository: Arc::new(InMemoryAiotDeviceSessionRepository::new()),
            credential_repository: Arc::new(InMemoryAiotCredentialRepository::new()),
            firmware_repository: Arc::new(AiotFirmwareRepositoryHandle::new_in_memory()),
            catalog_repository: Arc::new(AiotCatalogRepositoryHandle::new_in_memory()),
        }
    }

    pub fn surface(&self) -> AiotApiSurface {
        self.surface
    }

    pub fn runtime(&self) -> &AiotRuntime {
        &self.runtime
    }

    pub fn with_device_repository(
        mut self,
        device_repository: Arc<dyn AiotDeviceRepository>,
    ) -> Self {
        self.device_repository = device_repository;
        self
    }

    pub fn with_command_repository(
        mut self,
        command_repository: Arc<dyn AiotCommandRepository>,
    ) -> Self {
        self.command_repository = command_repository;
        self
    }

    pub fn with_event_repository(mut self, event_repository: Arc<dyn AiotEventRepository>) -> Self {
        self.event_repository = event_repository;
        self
    }

    pub fn with_twin_repository(
        mut self,
        twin_repository: Arc<dyn AiotDeviceTwinRepository>,
    ) -> Self {
        self.twin_repository = twin_repository;
        self
    }

    pub fn with_firmware_repository(
        mut self,
        firmware_repository: Arc<AiotFirmwareRepositoryHandle>,
    ) -> Self {
        self.firmware_repository = firmware_repository;
        self
    }

    pub fn with_credential_repository(
        mut self,
        credential_repository: Arc<dyn AiotCredentialRepository>,
    ) -> Self {
        self.credential_repository = credential_repository;
        self
    }

    pub fn with_device_session_repository(
        mut self,
        device_session_repository: Arc<dyn AiotDeviceSessionRepository>,
    ) -> Self {
        self.device_session_repository = device_session_repository;
        self
    }

    pub fn with_catalog_repository(
        mut self,
        catalog_repository: Arc<AiotCatalogRepositoryHandle>,
    ) -> Self {
        self.catalog_repository = catalog_repository;
        self
    }

    fn is_ready(&self) -> bool {
        !self.runtime.component_names().is_empty()
            && self.device_repository.storage_ready()
            && sdkwork_aiot_storage_sqlx::outbox_ready_from_env()
    }

    fn ensure_standard_catalog_seeded(&self, association: &AiotStorageAssociation) {
        for record in standard_capability_model_records() {
            if self
                .catalog_repository
                .get_capability_model(association, &record.capability_model_id)
                .is_some()
            {
                continue;
            }
            let _ = self.catalog_repository.create_capability_model(
                association.clone(),
                AiotCapabilityModelCreatePayload {
                    capability_model_id: record.capability_model_id,
                    display_name: record.display_name,
                    version: record.version,
                    capabilities: record.capabilities,
                },
            );
        }

        for record in standard_hardware_profile_records() {
            if self
                .catalog_repository
                .get_hardware_profile(association, &record.hardware_profile_id)
                .is_some()
            {
                continue;
            }
            let _ = self.catalog_repository.create_hardware_profile(
                association.clone(),
                AiotHardwareProfileCreatePayload {
                    hardware_profile_id: record.hardware_profile_id,
                    chip_family: record.chip_family,
                    hardware_classes: record.hardware_classes,
                    runtime_profiles: record.runtime_profiles,
                    connectivity_profiles: record.connectivity_profiles,
                    security_profiles: record.security_profiles,
                    ota_profiles: record.ota_profiles,
                },
            );
        }

        for record in standard_protocol_profile_records() {
            if self
                .catalog_repository
                .get_protocol_profile(association, &record.protocol_profile_id)
                .is_some()
            {
                continue;
            }
            let _ = self.catalog_repository.create_protocol_profile(
                association.clone(),
                AiotProtocolProfileCreatePayload {
                    protocol_profile_id: record.protocol_profile_id,
                    default_protocol_id: record.default_protocol_id,
                    scope: record.scope,
                    allowed_transports: record.allowed_transports,
                    allowed_message_classes: record.allowed_message_classes,
                    capability_bridges: record.capability_bridges,
                },
            );
        }

        for record in standard_product_records() {
            if self
                .catalog_repository
                .get_product(association, &record.product_id)
                .is_some()
            {
                continue;
            }
            let _ = self.catalog_repository.create_product(
                association.clone(),
                AiotProductCreatePayload {
                    product_id: record.product_id,
                    display_name: record.display_name,
                    default_hardware_profile_id: record.default_hardware_profile_id,
                    default_protocol_profile_id: record.default_protocol_profile_id,
                    default_capability_model_id: record.default_capability_model_id,
                },
            );
        }
    }

    fn create_product(
        &self,
        context: &AiotRequestContext,
        payload: AiotProductCreatePayload,
    ) -> Result<AiotProductRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .create_product(association, payload)
            .map_err(catalog_repository_error_to_response)
    }

    fn list_products(
        &self,
        context: &AiotRequestContext,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotProductRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.ensure_standard_catalog_seeded(&association);
        self.catalog_repository
            .list_products_page(&association, params)
            .map_err(catalog_repository_error_to_response)
    }

    fn get_product(
        &self,
        context: &AiotRequestContext,
        product_id: &str,
    ) -> Result<AiotProductRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.ensure_standard_catalog_seeded(&association);
        self.catalog_repository
            .get_product(&association, product_id)
            .ok_or_else(|| product_not_found_response(product_id))
    }

    fn update_product(
        &self,
        context: &AiotRequestContext,
        product_id: &str,
        payload: AiotProductUpdatePayload,
    ) -> Result<AiotProductRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .update_product(association, product_id, payload)
            .map_err(catalog_repository_error_to_response)
    }

    fn delete_product(
        &self,
        context: &AiotRequestContext,
        product_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .delete_product(&association, product_id)
            .map_err(catalog_repository_error_to_response)
    }

    fn create_hardware_profile(
        &self,
        context: &AiotRequestContext,
        payload: AiotHardwareProfileCreatePayload,
    ) -> Result<AiotHardwareProfileRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .create_hardware_profile(association, payload)
            .map_err(catalog_repository_error_to_response)
    }

    fn list_hardware_profiles(
        &self,
        context: &AiotRequestContext,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotHardwareProfileRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.ensure_standard_catalog_seeded(&association);
        self.catalog_repository
            .list_hardware_profiles_page(&association, params)
            .map_err(catalog_repository_error_to_response)
    }

    fn get_hardware_profile(
        &self,
        context: &AiotRequestContext,
        hardware_profile_id: &str,
    ) -> Result<AiotHardwareProfileRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.ensure_standard_catalog_seeded(&association);
        self.catalog_repository
            .get_hardware_profile(&association, hardware_profile_id)
            .ok_or_else(|| hardware_profile_not_found_response(hardware_profile_id))
    }

    fn update_hardware_profile(
        &self,
        context: &AiotRequestContext,
        hardware_profile_id: &str,
        payload: AiotHardwareProfileUpdatePayload,
    ) -> Result<AiotHardwareProfileRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .update_hardware_profile(association, hardware_profile_id, payload)
            .map_err(catalog_repository_error_to_response)
    }

    fn delete_hardware_profile(
        &self,
        context: &AiotRequestContext,
        hardware_profile_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .delete_hardware_profile(&association, hardware_profile_id)
            .map_err(catalog_repository_error_to_response)
    }

    fn create_protocol_profile(
        &self,
        context: &AiotRequestContext,
        payload: AiotProtocolProfileCreatePayload,
    ) -> Result<AiotProtocolProfileRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .create_protocol_profile(association, payload)
            .map_err(catalog_repository_error_to_response)
    }

    fn list_protocol_profiles(
        &self,
        context: &AiotRequestContext,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotProtocolProfileRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.ensure_standard_catalog_seeded(&association);
        self.catalog_repository
            .list_protocol_profiles_page(&association, params)
            .map_err(catalog_repository_error_to_response)
    }

    fn get_protocol_profile(
        &self,
        context: &AiotRequestContext,
        protocol_profile_id: &str,
    ) -> Result<AiotProtocolProfileRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.ensure_standard_catalog_seeded(&association);
        self.catalog_repository
            .get_protocol_profile(&association, protocol_profile_id)
            .ok_or_else(|| protocol_profile_not_found_response(protocol_profile_id))
    }

    fn update_protocol_profile(
        &self,
        context: &AiotRequestContext,
        protocol_profile_id: &str,
        payload: AiotProtocolProfileUpdatePayload,
    ) -> Result<AiotProtocolProfileRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .update_protocol_profile(association, protocol_profile_id, payload)
            .map_err(catalog_repository_error_to_response)
    }

    fn delete_protocol_profile(
        &self,
        context: &AiotRequestContext,
        protocol_profile_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .delete_protocol_profile(&association, protocol_profile_id)
            .map_err(catalog_repository_error_to_response)
    }

    fn create_capability_model(
        &self,
        context: &AiotRequestContext,
        payload: AiotCapabilityModelCreatePayload,
    ) -> Result<AiotCapabilityModelRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .create_capability_model(association, payload)
            .map_err(catalog_repository_error_to_response)
    }

    fn get_capability_model(
        &self,
        context: &AiotRequestContext,
        capability_model_id: &str,
    ) -> Result<AiotCapabilityModelRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.ensure_standard_catalog_seeded(&association);
        self.catalog_repository
            .get_capability_model(&association, capability_model_id)
            .ok_or_else(|| capability_model_not_found_response(capability_model_id))
    }

    fn list_capability_models(
        &self,
        context: &AiotRequestContext,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotCapabilityModelRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.ensure_standard_catalog_seeded(&association);
        self.catalog_repository
            .list_capability_models_page(&association, params)
            .map_err(catalog_repository_error_to_response)
    }

    fn update_capability_model(
        &self,
        context: &AiotRequestContext,
        capability_model_id: &str,
        payload: AiotCapabilityModelUpdatePayload,
    ) -> Result<AiotCapabilityModelRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .update_capability_model(association, capability_model_id, payload)
            .map_err(catalog_repository_error_to_response)
    }

    fn delete_capability_model(
        &self,
        context: &AiotRequestContext,
        capability_model_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.catalog_repository
            .delete_capability_model(&association, capability_model_id)
            .map_err(catalog_repository_error_to_response)
    }

    fn create_device(
        &self,
        context: &AiotRequestContext,
        payload: AiotDeviceCreatePayload,
    ) -> Result<AiotDeviceRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        let mut command = AiotDeviceCreateCommand::new(
            association,
            payload.device_id,
            payload.display_name,
            payload.product_id,
        );
        if let Some(client_id) = payload.client_id {
            command = command.with_client_id(client_id);
        }
        if let Some(chip_family) = payload.chip_family {
            command = command.with_chip_family(chip_family);
        }

        self.device_repository
            .create_device(command)
            .map_err(device_repository_error_to_response)
    }

    fn list_devices(
        &self,
        context: &AiotRequestContext,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.device_repository
            .list_devices(&association, params)
            .map_err(device_repository_error_to_response)
    }

    fn get_device(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
    ) -> Option<AiotDeviceRecord> {
        request_context_to_storage_association(context)
            .ok()
            .and_then(|association| self.device_repository.get_device(&association, device_id))
    }

    fn update_device(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        payload: AiotDeviceUpdatePayload,
    ) -> Result<AiotDeviceRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        let mut command = AiotDeviceUpdateCommand::new(association, device_id.to_string());
        if let Some(display_name) = payload.display_name {
            command = command.with_display_name(display_name);
        }
        if let Some(status) = payload.status {
            command = command.with_status(status);
        }
        if let Some(metadata_json) = payload.metadata_json {
            command = command.with_metadata_json(metadata_json);
        }
        self.device_repository
            .update_device(command)
            .map_err(device_repository_error_to_response)
    }

    fn delete_device(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.device_repository
            .delete_device(&association, device_id)
            .map_err(device_repository_error_to_response)
    }

    fn create_command(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        payload: AiotCommandCreatePayload,
    ) -> Result<AiotCommandRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        let mut command = AiotCommandCreateCommand::new(
            association.clone(),
            device_id.to_string(),
            payload.capability_name,
            payload.command_name,
        )
        .with_request_payload_json(payload.payload_json)
        .with_request_media(
            payload.request_media_resource_id,
            payload.request_object_blob_id,
            payload.request_media_json,
        );
        if let Some(trace_id) = payload.trace_id {
            command = command.with_trace_id(trace_id);
        }
        if let Some(idempotency_key) = payload.idempotency_key {
            command = command.with_idempotency_key(idempotency_key);
        }
        if let Some(timeout_at) = payload.timeout_at {
            command = command.with_timeout_at(timeout_at);
        }
        if let Some(session_id) = payload.session_id {
            command = command.with_session_id(session_id);
        }

        let record = self
            .command_repository
            .create_command(command)
            .map_err(command_repository_error_to_response)?;

        self.maybe_process_assistant_chat_kernel(&association, &record)?;

        Ok(record)
    }

    fn list_commands(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotCommandRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.command_repository
            .list_commands(&association, device_id, params)
            .map_err(command_repository_error_to_response)
    }

    fn get_command(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        command_id: &str,
    ) -> Result<AiotCommandRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.command_repository
            .get_command(&association, device_id, command_id)
            .map_err(command_repository_error_to_response)?
            .ok_or_else(|| command_not_found_response(command_id))
    }

    fn list_device_sessions(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceSessionRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        if self
            .device_repository
            .get_device(&association, device_id)
            .is_none()
        {
            return Err(device_not_found_response(device_id));
        }

        self.device_session_repository
            .list_sessions(&association, device_id, params)
            .map_err(device_repository_error_to_response)
    }

    fn disconnect_device_session(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        session_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        if self
            .device_repository
            .get_device(&association, device_id)
            .is_none()
        {
            return Err(device_not_found_response(device_id));
        }
        let disconnected = self
            .device_session_repository
            .disconnect_session(&association, device_id, session_id)
            .map_err(device_repository_error_to_response)?;
        if !disconnected {
            return Err(device_session_not_found_response(session_id));
        }
        Ok(())
    }

    fn list_device_capabilities(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceCapabilityRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        let device = self
            .device_repository
            .get_device(&association, device_id)
            .ok_or_else(|| device_not_found_response(device_id))?;

        let product = self.get_product(context, &device.product_id)?;
        let capability_model =
            self.get_capability_model(context, &product.default_capability_model_id)?;

        let capabilities = capability_model
            .capabilities
            .iter()
            .map(|definition| AiotDeviceCapabilityRecord {
                capability_name: definition.name.clone(),
                capability_kind: capability_kind_name(definition.kind).to_string(),
                status: "enabled".to_string(),
            })
            .collect::<Vec<_>>();

        Ok(paginate_bounded_catalog(capabilities, params))
    }

    fn list_events(
        &self,
        context: &AiotRequestContext,
        device_id: Option<&str>,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceEventRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.event_repository
            .list_events(&association, device_id, params)
            .map_err(event_repository_error_to_response)
    }

    fn get_twin_snapshot(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
    ) -> Result<AiotDeviceTwinSnapshot, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.twin_repository
            .get_twin_snapshot(&association, device_id)
            .map_err(twin_repository_error_to_response)
    }

    fn update_twin(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        payload: AiotTwinUpdatePayload,
    ) -> Result<AiotDeviceTwinSnapshot, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        if self
            .device_repository
            .get_device(&association, device_id)
            .is_none()
        {
            return Err(device_not_found_response(device_id));
        }

        let mut latest = self
            .twin_repository
            .get_twin_snapshot(&association, device_id)
            .map_err(twin_repository_error_to_response)?;
        for (key, value_json) in payload.desired {
            latest = self
                .twin_repository
                .upsert_twin_property(
                    AiotTwinPropertyUpsertCommand::new(association.clone(), device_id, key)
                        .with_desired_value_json(value_json),
                )
                .map_err(twin_repository_error_to_response)?;
        }
        for (key, value_json) in payload.reported {
            latest = self
                .twin_repository
                .upsert_twin_property(
                    AiotTwinPropertyUpsertCommand::new(association.clone(), device_id, key)
                        .with_reported_value_json(value_json),
                )
                .map_err(twin_repository_error_to_response)?;
        }
        Ok(latest)
    }

    fn list_device_credentials(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceCredentialRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        if self
            .device_repository
            .get_device(&association, device_id)
            .is_none()
        {
            return Err(device_not_found_response(device_id));
        }
        let credentials =
            self.credential_repository
                .list_credentials(&association, device_id, params)?;
        Ok(credentials)
    }

    fn create_device_credential(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        payload: AiotCredentialCreatePayload,
    ) -> Result<AiotDeviceCredentialRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        if self
            .device_repository
            .get_device(&association, device_id)
            .is_none()
        {
            return Err(device_not_found_response(device_id));
        }

        self.credential_repository.create_credential(
            association,
            AiotCredentialCreateCommand {
                device_id: device_id.to_string(),
                credential_type: payload.credential_type,
                expires_at: payload.expires_at,
            },
        )
    }

    fn get_device_credential(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        credential_id: &str,
    ) -> Result<AiotDeviceCredentialRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        if self
            .device_repository
            .get_device(&association, device_id)
            .is_none()
        {
            return Err(device_not_found_response(device_id));
        }
        self.credential_repository
            .get_credential(&association, device_id, credential_id)
            .ok_or_else(|| credential_not_found_response(credential_id))
    }

    fn delete_device_credential(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        credential_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        if self
            .device_repository
            .get_device(&association, device_id)
            .is_none()
        {
            return Err(device_not_found_response(device_id));
        }

        self.credential_repository
            .delete_credential(&association, device_id, credential_id)
    }

    fn cancel_command(
        &self,
        context: &AiotRequestContext,
        device_id: &str,
        command_id: &str,
    ) -> Result<AiotCommandRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        if self
            .device_repository
            .get_device(&association, device_id)
            .is_none()
        {
            return Err(device_not_found_response(device_id));
        }
        self.command_repository
            .cancel_command(&association, device_id, command_id)
            .map_err(command_repository_error_to_response)?
            .ok_or_else(|| command_not_found_response(command_id))
    }

    fn create_firmware_artifact(
        &self,
        context: &AiotRequestContext,
        payload: AiotFirmwareArtifactCreatePayload,
    ) -> Result<AiotFirmwareArtifactRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .create_artifact(association, payload)
            .map_err(firmware_repository_error_to_response)
    }

    fn list_firmware_artifacts(
        &self,
        context: &AiotRequestContext,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotFirmwareArtifactRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .list_artifacts_page(&association, params)
            .map_err(firmware_repository_error_to_response)
    }

    fn get_firmware_artifact(
        &self,
        context: &AiotRequestContext,
        artifact_id: &str,
    ) -> Result<AiotFirmwareArtifactRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .get_artifact(&association, artifact_id)
            .ok_or_else(|| firmware_artifact_not_found_response(artifact_id))
    }

    fn update_firmware_artifact(
        &self,
        context: &AiotRequestContext,
        artifact_id: &str,
        payload: AiotFirmwareArtifactUpdatePayload,
    ) -> Result<AiotFirmwareArtifactRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .update_artifact(association, artifact_id, payload)
            .map_err(firmware_repository_error_to_response)
    }

    fn delete_firmware_artifact(
        &self,
        context: &AiotRequestContext,
        artifact_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .delete_artifact(&association, artifact_id)
            .map_err(firmware_repository_error_to_response)
    }

    fn create_firmware_rollout(
        &self,
        context: &AiotRequestContext,
        payload: AiotFirmwareRolloutCreatePayload,
    ) -> Result<AiotFirmwareRolloutRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        let target_device_ids = resolve_rollout_target_device_ids(
            &payload.target_policy_json,
            &association,
            self.device_repository.as_ref(),
        )
        .map_err(|error| match error {
            RolloutTargetPolicyError::InvalidJson => problem_response(
                HttpStatus::BadRequest,
                "api.firmware.rollout.invalid_target_policy",
                "Firmware rollout target policy JSON is invalid",
            ),
        })?;
        self.firmware_repository
            .create_rollout(association, payload, &target_device_ids)
            .map_err(firmware_repository_error_to_response)
    }

    fn list_firmware_rollouts(
        &self,
        context: &AiotRequestContext,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotFirmwareRolloutRecord>, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .list_rollouts_page(&association, params)
            .map_err(firmware_repository_error_to_response)
    }

    fn get_firmware_rollout(
        &self,
        context: &AiotRequestContext,
        rollout_id: &str,
    ) -> Result<AiotFirmwareRolloutRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .get_rollout(&association, rollout_id)
            .ok_or_else(|| firmware_rollout_not_found_response(rollout_id))
    }

    fn update_firmware_rollout(
        &self,
        context: &AiotRequestContext,
        rollout_id: &str,
        payload: AiotFirmwareRolloutUpdatePayload,
    ) -> Result<AiotFirmwareRolloutRecord, HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .update_rollout(association, rollout_id, payload)
            .map_err(firmware_repository_error_to_response)
    }

    fn delete_firmware_rollout(
        &self,
        context: &AiotRequestContext,
        rollout_id: &str,
    ) -> Result<(), HttpResponse> {
        let association = request_context_to_storage_association(context)?;
        self.firmware_repository
            .delete_rollout(&association, rollout_id)
            .map_err(firmware_repository_error_to_response)
    }

    fn maybe_process_assistant_chat_kernel(
        &self,
        association: &AiotStorageAssociation,
        command: &AiotCommandRecord,
    ) -> Result<(), HttpResponse> {
        if command.capability_name != "assistant" || command.command_name != "chat" {
            return Ok(());
        }

        #[cfg(not(feature = "intelligence-kernel"))]
        {
            return Err(problem_response(
                HttpStatus::InternalServerError,
                "api.intelligence.kernel_unavailable",
                "Intelligence kernel integration is not enabled in this server build",
            ));
        }

        #[cfg(feature = "intelligence-kernel")]
        {
            use sdkwork_aiot_intelligence_bridge::{
                is_kernel_mode, KernelRuntimeClient, SessionMap, DEFAULT_KERNEL_AGENT_ID,
                ENV_INTELLIGENCE_KERNEL_AGENT_ID, ENV_INTELLIGENCE_KERNEL_HTTP_URL,
                KERNEL_PUBLIC_HTTP_URL_FALLBACK_ENV,
            };
            use sdkwork_aiot_storage::AiotDeviceEventCreateCommand;

            if !is_kernel_mode() {
                return Ok(());
            }

            let user_text = assistant_chat_user_text(&command.request_payload_json);
            if user_text.is_empty() {
                return Ok(());
            }

            let kernel_http_url = std::env::var(ENV_INTELLIGENCE_KERNEL_HTTP_URL)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    std::env::var(KERNEL_PUBLIC_HTTP_URL_FALLBACK_ENV)
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                })
                .ok_or_else(|| {
                    problem_response(
                        HttpStatus::InternalServerError,
                        "api.intelligence.kernel_misconfigured",
                        "Kernel runtime HTTP URL is not configured",
                    )
                })?;

            let agent_id = std::env::var(ENV_INTELLIGENCE_KERNEL_AGENT_ID)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_KERNEL_AGENT_ID.to_string());

            let kernel = KernelRuntimeClient::new(kernel_http_url).map_err(|error| {
                problem_response(
                    HttpStatus::InternalServerError,
                    "api.intelligence.kernel_unavailable",
                    &error,
                )
            })?;
            let session_map = SessionMap::new();
            let xiaozhi_session_id = command
                .session_id
                .as_deref()
                .unwrap_or(command.device_id.as_str());
            let kernel_session_id = kernel
                .ensure_session(&session_map, xiaozhi_session_id, &agent_id)
                .map_err(|error| {
                    problem_response(
                        HttpStatus::InternalServerError,
                        "api.intelligence.kernel_session_failed",
                        &error,
                    )
                })?;
            let reply = kernel
                .send_user_message(&kernel_session_id, &user_text)
                .map_err(|error| {
                    problem_response(
                        HttpStatus::InternalServerError,
                        "api.intelligence.kernel_message_failed",
                        &error,
                    )
                })?;

            let occurred_at = current_rfc3339_timestamp();
            let event_payload = serde_json::json!({
                "commandId": command.command_id,
                "capabilityName": command.capability_name,
                "commandName": command.command_name,
                "status": "completed",
                "sessionId": command.session_id,
                "traceId": command.trace_id,
                "result": {
                    "resultPayload": { "text": reply },
                    "occurredAt": occurred_at,
                }
            })
            .to_string();

            let mut event_command = AiotDeviceEventCreateCommand::new(
                association.clone(),
                &command.device_id,
                "iot.command.resultRecorded",
            )
            .with_message_routing("command", "result", "http", "cloud_to_app")
            .with_payload_json(event_payload);
            if let Some(trace_id) = command.trace_id.as_deref() {
                event_command = event_command.with_trace_id(trace_id);
            }

            self.event_repository
                .record_event(event_command)
                .map_err(event_repository_error_to_response)?;
        }

        Ok(())
    }
}

pub fn standard_admin_api_server() -> Result<AiotApiServer, RuntimeBuildError> {
    Ok(AiotApiServer::new(
        AiotApiSurface::Admin,
        standard_aiot_runtime(RuntimeMode::Standalone)?,
    ))
}

pub fn standard_app_api_server() -> Result<AiotApiServer, RuntimeBuildError> {
    Ok(AiotApiServer::new(
        AiotApiSurface::App,
        standard_aiot_runtime(RuntimeMode::Standalone)?,
    ))
}

pub fn handle_api_request_bytes(
    server: &AiotApiServer,
    bytes: &[u8],
) -> Result<String, AiotApiError> {
    let request = parse_http_request(bytes)?;
    let response = handle_api_request(server, &request);
    Ok(format_http_response(&response))
}

pub fn format_api_error_response(code: &str) -> String {
    format_http_response(&problem_response(
        HttpStatus::BadRequest,
        code,
        "Bad Request",
    ))
}

pub fn handle_api_request(server: &AiotApiServer, request: &HttpRequest) -> HttpResponse {
    if let Some(response) = cors_preflight_response(request) {
        return response;
    }

    let resolved = match resolve_api_request(request) {
        Ok(resolved) => resolved,
        Err(response) => return apply_cors_headers(request, response),
    };

    let response = apply_cors_headers(request, handle_resolved_api_request(server, &resolved));
    emit_api_request_trace(&request.method, &request.path, response.status.code());
    response
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiotApiRequestContext {
    Public,
    Protected(Box<AiotRequestContext>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotResolvedApiRequest<'a> {
    request: &'a HttpRequest,
    context: AiotApiRequestContext,
}

impl<'a> AiotResolvedApiRequest<'a> {
    pub fn public(request: &'a HttpRequest) -> Self {
        Self {
            request,
            context: AiotApiRequestContext::Public,
        }
    }

    pub fn protected(request: &'a HttpRequest, context: AiotRequestContext) -> Self {
        Self {
            request,
            context: AiotApiRequestContext::Protected(Box::new(context)),
        }
    }

    pub fn request(&self) -> &HttpRequest {
        self.request
    }

    pub fn context(&self) -> &AiotApiRequestContext {
        &self.context
    }
}

pub fn resolve_api_request(
    request: &HttpRequest,
) -> Result<AiotResolvedApiRequest<'_>, HttpResponse> {
    if is_protected_iot_api_path(&request.path) {
        return resolve_protected_request_context(request)
            .map(|ctx| AiotResolvedApiRequest::protected(request, ctx));
    }

    Ok(AiotResolvedApiRequest::public(request))
}

/// Resolves API request context from an SDKWork web-framework [`WebRequestContext`].
///
/// Used by Axum routers wrapped with `sdkwork-web-framework`; legacy byte transport keeps
/// [`resolve_api_request`] and [`DefaultSdkworkIamContextResolver`].
pub fn resolve_api_request_from_web_context<'a>(
    request: &'a HttpRequest,
    web_context: &sdkwork_web_core::WebRequestContext,
) -> Result<AiotResolvedApiRequest<'a>, HttpResponse> {
    if is_protected_iot_api_path(&request.path) {
        return match sdkwork_aiot_app_context::aiot_context_from_web_request(web_context) {
            Some(context) => Ok(AiotResolvedApiRequest::protected(request, context)),
            None => Err(problem_response(
                HttpStatus::Forbidden,
                "api.context.missing",
                "Resolved SDKWork request context is required",
            )),
        };
    }

    Ok(AiotResolvedApiRequest::public(request))
}

pub fn handle_resolved_api_request(
    server: &AiotApiServer,
    resolved: &AiotResolvedApiRequest<'_>,
) -> HttpResponse {
    let request = resolved.request();
    if is_protected_iot_api_path(&request.path)
        && !matches!(resolved.context(), AiotApiRequestContext::Protected(_))
    {
        return problem_response(
            HttpStatus::Forbidden,
            "api.context.missing",
            "Resolved appbase context is required",
        );
    }
    if let Err(response) = enforce_route_permission(server.surface, resolved) {
        return response;
    }

    if matches!(request.path.as_str(), "/healthz" | "/readyz") {
        let ready = if request.path.as_str() == "/healthz" {
            true
        } else {
            server.is_ready()
        };
        return build_health_response("sdkwork-iot-platform-service", ready);
    }

    let Some(route) = route_contract_for_request(server.surface, request) else {
        return problem_response(
            HttpStatus::NotFound,
            "api.route.unsupported",
            "API route is not mounted on this surface",
        );
    };

    let product_id = route_parameter_value(route.path, &request.path, "productId");
    let hardware_profile_id = route_parameter_value(route.path, &request.path, "hardwareProfileId");
    let protocol_profile_id = route_parameter_value(route.path, &request.path, "protocolProfileId");
    let device_id = route_parameter_value(route.path, &request.path, "deviceId");
    let capability_model_id = route_parameter_value(route.path, &request.path, "capabilityModelId");
    let artifact_id = route_parameter_value(route.path, &request.path, "artifactId");
    let rollout_id = route_parameter_value(route.path, &request.path, "rolloutId");
    let credential_id = route_parameter_value(route.path, &request.path, "credentialId");
    let session_id = route_parameter_value(route.path, &request.path, "sessionId");
    let command_id = route_parameter_value(route.path, &request.path, "commandId");
    let request_context = match resolved.context() {
        AiotApiRequestContext::Protected(context) => Some(context.as_ref()),
        AiotApiRequestContext::Public => None,
    };

    match (server.surface, route.operation_id) {
        (AiotApiSurface::Admin, "protocolAdapters.list") => {
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            let items = protocol_adapter_item_json(server.runtime());
            let page = paginate_bounded_catalog(items, page_params);
            json_collection_response(request, &page.items.join(","), page_params, page.total)
        }
        (AiotApiSurface::Admin, "runtime.capacity.retrieve") => {
            standard_resource_response(request, HttpStatus::Ok, runtime_capacity_data_json())
        }
        (AiotApiSurface::Admin, "products.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_products(context, page_params) {
                Ok(page) => standard_product_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "products.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let payload = match product_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_product(context, payload) {
                Ok(record) => standard_product_response(request, HttpStatus::Created, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "products.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let product_id = product_id.as_deref().unwrap_or("unknown-product");
            match server.get_product(context, product_id) {
                Ok(record) => standard_product_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "products.update") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let product_id = product_id.as_deref().unwrap_or("unknown-product");
            let payload = match product_update_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.update_product(context, product_id, payload) {
                Ok(record) => standard_product_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "products.delete") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let product_id = product_id.as_deref().unwrap_or("unknown-product");
            match server.delete_product(context, product_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "hardwareProfiles.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_hardware_profiles(context, page_params) {
                Ok(page) => standard_hardware_profile_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "hardwareProfiles.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let payload = match hardware_profile_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_hardware_profile(context, payload) {
                Ok(record) => {
                    standard_hardware_profile_response(request, HttpStatus::Created, &record)
                }
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "hardwareProfiles.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let hardware_profile_id = hardware_profile_id
                .as_deref()
                .unwrap_or("unknown-hardware-profile");
            match server.get_hardware_profile(context, hardware_profile_id) {
                Ok(record) => standard_hardware_profile_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "hardwareProfiles.update") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let hardware_profile_id = hardware_profile_id
                .as_deref()
                .unwrap_or("unknown-hardware-profile");
            let payload = match hardware_profile_update_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.update_hardware_profile(context, hardware_profile_id, payload) {
                Ok(record) => standard_hardware_profile_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "hardwareProfiles.delete") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let hardware_profile_id = hardware_profile_id
                .as_deref()
                .unwrap_or("unknown-hardware-profile");
            match server.delete_hardware_profile(context, hardware_profile_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "protocolProfiles.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_protocol_profiles(context, page_params) {
                Ok(page) => standard_protocol_profile_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "protocolProfiles.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let payload = match protocol_profile_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_protocol_profile(context, payload) {
                Ok(record) => {
                    standard_protocol_profile_response(request, HttpStatus::Created, &record)
                }
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "protocolProfiles.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let protocol_profile_id = protocol_profile_id
                .as_deref()
                .unwrap_or("unknown-protocol-profile");
            match server.get_protocol_profile(context, protocol_profile_id) {
                Ok(record) => standard_protocol_profile_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "protocolProfiles.update") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let protocol_profile_id = protocol_profile_id
                .as_deref()
                .unwrap_or("unknown-protocol-profile");
            let payload = match protocol_profile_update_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.update_protocol_profile(context, protocol_profile_id, payload) {
                Ok(record) => standard_protocol_profile_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "protocolProfiles.delete") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let protocol_profile_id = protocol_profile_id
                .as_deref()
                .unwrap_or("unknown-protocol-profile");
            match server.delete_protocol_profile(context, protocol_profile_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "capabilityModels.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_capability_models(context, page_params) {
                Ok(page) => standard_capability_model_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "capabilityModels.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let payload = match capability_model_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_capability_model(context, payload) {
                Ok(record) => {
                    standard_capability_model_record_response(request, HttpStatus::Created, &record)
                }
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.list") | (AiotApiSurface::App, "devices.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_devices(context, page_params) {
                Ok(page) => standard_device_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.sessions.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_device_sessions(context, device_id, page_params) {
                Ok(page) => standard_device_session_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.sessions.disconnect") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let session_id = session_id.as_deref().unwrap_or("unknown-session");
            match server.disconnect_device_session(context, device_id, session_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.capabilities.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_device_capabilities(context, device_id, page_params) {
                Ok(page) => standard_device_capability_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.commands.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_commands(context, device_id, page_params) {
                Ok(page) => standard_command_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.commands.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let command_id = command_id.as_deref().unwrap_or("unknown-command");
            match server.get_command(context, device_id, command_id) {
                Ok(command) => standard_command_response(request, HttpStatus::Ok, &command),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.commands.cancel") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let command_id = command_id.as_deref().unwrap_or("unknown-command");
            match server.cancel_command(context, device_id, command_id) {
                Ok(command) => standard_command_acceptance_response(
                    request,
                    HttpStatus::Ok,
                    &command.command_id,
                    Some(command.status.as_str()),
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "events.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_events(context, None, page_params) {
                Ok(page) => standard_event_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::App, "devices.events.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_events(context, Some(device_id), page_params) {
                Ok(page) => standard_event_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "capabilityModels.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let capability_model_id = capability_model_id
                .as_deref()
                .unwrap_or("unknown-capability-model");
            match server.get_capability_model(context, capability_model_id) {
                Ok(record) => {
                    standard_capability_model_record_response(request, HttpStatus::Ok, &record)
                }
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "capabilityModels.update") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let capability_model_id = capability_model_id
                .as_deref()
                .unwrap_or("unknown-capability-model");
            let payload = match capability_model_update_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.update_capability_model(context, capability_model_id, payload) {
                Ok(record) => {
                    standard_capability_model_record_response(request, HttpStatus::Ok, &record)
                }
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "capabilityModels.delete") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let capability_model_id = capability_model_id
                .as_deref()
                .unwrap_or("unknown-capability-model");
            match server.delete_capability_model(context, capability_model_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let payload = match device_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_device(context, payload) {
                Ok(device) => standard_device_response(request, HttpStatus::Created, &device),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.retrieve") | (AiotApiSurface::App, "devices.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            match server.get_device(context, device_id) {
                Some(device) => standard_device_response(request, HttpStatus::Ok, &device),
                None => device_not_found_response(device_id),
            }
        }
        (AiotApiSurface::Admin, "devices.update") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let payload = match device_update_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.update_device(context, device_id, payload) {
                Ok(device) => standard_device_response(request, HttpStatus::Ok, &device),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.delete") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            match server.delete_device(context, device_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.credentials.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_device_credentials(context, device_id, page_params) {
                Ok(page) => standard_device_credential_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.credentials.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let credential_id = credential_id.as_deref().unwrap_or("unknown-credential");
            match server.get_device_credential(context, device_id, credential_id) {
                Ok(record) => standard_device_credential_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.credentials.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let payload = match credential_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_device_credential(context, device_id, payload) {
                Ok(record) => {
                    standard_device_credential_response(request, HttpStatus::Created, &record)
                }
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.credentials.delete") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let credential_id = credential_id.as_deref().unwrap_or("unknown-credential");
            match server.delete_device_credential(context, device_id, credential_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.twin.retrieve")
        | (AiotApiSurface::App, "devices.twin.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            match server.get_twin_snapshot(context, device_id) {
                Ok(snapshot) => standard_twin_response(request, &snapshot),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "devices.twin.update") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let payload = match twin_update_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.update_twin(context, device_id, payload) {
                Ok(snapshot) => standard_twin_response(request, &snapshot),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::App, "devices.commands.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let command_payload = match command_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_command(context, device_id, command_payload) {
                Ok(command) => standard_command_acceptance_response(
                    request,
                    HttpStatus::Accepted,
                    &command.command_id,
                    Some(command.status.as_str()),
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::App, "devices.commands.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let device_id = device_id.as_deref().unwrap_or("unknown-device");
            let command_id = command_id.as_deref().unwrap_or("unknown-command");
            match server.get_command(context, device_id, command_id) {
                Ok(command) => standard_command_response(request, HttpStatus::Ok, &command),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareArtifacts.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_firmware_artifacts(context, page_params) {
                Ok(page) => standard_firmware_artifact_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareArtifacts.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let payload = match firmware_artifact_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_firmware_artifact(context, payload) {
                Ok(record) => {
                    standard_firmware_artifact_response(request, HttpStatus::Created, &record)
                }
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareArtifacts.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let artifact_id = artifact_id.as_deref().unwrap_or("unknown-artifact");
            match server.get_firmware_artifact(context, artifact_id) {
                Ok(record) => standard_firmware_artifact_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareArtifacts.update") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let artifact_id = artifact_id.as_deref().unwrap_or("unknown-artifact");
            let payload = match firmware_artifact_update_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.update_firmware_artifact(context, artifact_id, payload) {
                Ok(record) => standard_firmware_artifact_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareArtifacts.delete") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let artifact_id = artifact_id.as_deref().unwrap_or("unknown-artifact");
            match server.delete_firmware_artifact(context, artifact_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareRollouts.list") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let page_params = match require_page_params(request) {
                Ok(params) => params,
                Err(response) => return response,
            };
            match server.list_firmware_rollouts(context, page_params) {
                Ok(page) => standard_firmware_rollout_collection_response(
                    request,
                    &page.items,
                    page_params,
                    page.total,
                ),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareRollouts.create") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let payload = match firmware_rollout_create_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.create_firmware_rollout(context, payload) {
                Ok(record) => {
                    standard_firmware_rollout_response(request, HttpStatus::Accepted, &record)
                }
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareRollouts.retrieve") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let rollout_id = rollout_id.as_deref().unwrap_or("unknown-rollout");
            match server.get_firmware_rollout(context, rollout_id) {
                Ok(record) => standard_firmware_rollout_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareRollouts.update") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let rollout_id = rollout_id.as_deref().unwrap_or("unknown-rollout");
            let payload = match firmware_rollout_update_payload_from_request(request) {
                Ok(payload) => payload,
                Err(problem) => return problem,
            };
            match server.update_firmware_rollout(context, rollout_id, payload) {
                Ok(record) => standard_firmware_rollout_response(request, HttpStatus::Ok, &record),
                Err(problem) => problem,
            }
        }
        (AiotApiSurface::Admin, "firmwareRollouts.delete") => {
            let Some(context) = request_context else {
                return problem_response(
                    HttpStatus::Forbidden,
                    "api.context.missing",
                    "Resolved appbase context is required",
                );
            };
            let rollout_id = rollout_id.as_deref().unwrap_or("unknown-rollout");
            match server.delete_firmware_rollout(context, rollout_id) {
                Ok(()) => HttpResponse::new(HttpStatus::NoContent),
                Err(problem) => problem,
            }
        }
        _ => problem_response(
            HttpStatus::NotFound,
            "api.route.unsupported",
            "API route is not mounted on this surface",
        ),
    }
}

fn is_protected_iot_api_path(path: &str) -> bool {
    path.starts_with("/backend/v3/api/iot") || path.starts_with("/app/v3/api/iot")
}

fn resolve_protected_request_context(
    request: &HttpRequest,
) -> Result<AiotRequestContext, HttpResponse> {
    default_iam_context_resolver().resolve(request)
}

fn enforce_route_permission(
    surface: AiotApiSurface,
    resolved: &AiotResolvedApiRequest<'_>,
) -> Result<(), HttpResponse> {
    let request = resolved.request();
    let Some(route) = route_contract_for_request(surface, request) else {
        return Ok(());
    };

    let AiotApiRequestContext::Protected(ctx) = resolved.context() else {
        return Err(problem_response(
            HttpStatus::Forbidden,
            "api.context.missing",
            "Resolved appbase context is required",
        ));
    };

    if ctx.has_permission(route.required_permission) {
        Ok(())
    } else {
        Err(permission_denied_response(route.required_permission))
    }
}

fn route_path_matches(template: &str, path: &str) -> bool {
    let template_segments = template.trim_matches('/').split('/').collect::<Vec<_>>();
    let path_segments = path.trim_matches('/').split('/').collect::<Vec<_>>();

    if template_segments.len() != path_segments.len() {
        return false;
    }

    template_segments
        .iter()
        .zip(path_segments.iter())
        .all(|(template, actual)| {
            (template.starts_with('{') && template.ends_with('}') && !actual.is_empty())
                || template == actual
        })
}

fn route_parameter_value(template: &str, path: &str, parameter_name: &str) -> Option<String> {
    let template_segments = template.trim_matches('/').split('/').collect::<Vec<_>>();
    let path_segments = path.trim_matches('/').split('/').collect::<Vec<_>>();

    if template_segments.len() != path_segments.len() {
        return None;
    }

    template_segments.iter().zip(path_segments.iter()).find_map(
        |(template_segment, path_segment)| {
            let param = template_segment
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))?;
            if param == parameter_name {
                Some((*path_segment).to_string())
            } else {
                None
            }
        },
    )
}

fn permission_scope_headers(request: &HttpRequest) -> Vec<&str> {
    optional_header(request, "x-sdkwork-permission-scope")
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect()
}

fn is_blank_header(request: &HttpRequest, name: &str) -> bool {
    optional_header(request, name).is_none()
}

fn required_header<'a>(request: &'a HttpRequest, name: &str) -> Result<&'a str, ()> {
    optional_header(request, name).ok_or(())
}

fn access_token_header(request: &HttpRequest) -> Option<&str> {
    optional_header(request, "access-token")
        .or_else(|| optional_header(request, "sdkwork-access-token"))
}

fn optional_header<'a>(request: &'a HttpRequest, name: &str) -> Option<&'a str> {
    request
        .header(name)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_i64(value: &str) -> Result<i64, std::num::ParseIntError> {
    value.parse::<i64>()
}

fn request_context_to_storage_association(
    context: &AiotRequestContext,
) -> Result<AiotStorageAssociation, HttpResponse> {
    let tenant_id = parse_i64(&context.tenant_id).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.context.invalid_tenant_id",
            "Resolved tenant id is invalid",
        )
    })?;
    let organization_id = parse_i64(&context.organization_id).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.context.invalid_organization_id",
            "Resolved organization id is invalid",
        )
    })?;

    let mut association = AiotStorageAssociation::tenant_org(tenant_id, organization_id);
    if let Some(user_id) = context.user_id.as_deref() {
        let user_id = parse_i64(user_id).map_err(|_| {
            problem_response(
                HttpStatus::BadRequest,
                "api.context.invalid_user_id",
                "Resolved user id is invalid",
            )
        })?;
        association = association.with_user_id(user_id);
    }

    Ok(association)
}

fn device_repository_error_to_response(error: AiotDeviceRepositoryError) -> HttpResponse {
    match error {
        AiotDeviceRepositoryError::DuplicateDeviceId => problem_response(
            HttpStatus::Conflict,
            "api.device.duplicate_device_id",
            "Device id already exists",
        ),
        AiotDeviceRepositoryError::InvalidProductId => problem_response(
            HttpStatus::BadRequest,
            "api.device.invalid_product_id",
            "Product id must be an int64 string",
        ),
        AiotDeviceRepositoryError::NotFound => {
            problem_response(HttpStatus::NotFound, "api.device.not_found", "Not Found")
        }
        AiotDeviceRepositoryError::PersistenceFailure => problem_response(
            HttpStatus::InternalServerError,
            "api.storage.write_failed",
            "Storage write failed",
        ),
    }
}

fn command_repository_error_to_response(error: AiotCommandRepositoryError) -> HttpResponse {
    match error {
        AiotCommandRepositoryError::DuplicateCommandId => problem_response(
            HttpStatus::Conflict,
            "api.command.duplicate_command_id",
            "Command id already exists",
        ),
        AiotCommandRepositoryError::PersistenceFailure => problem_response(
            HttpStatus::InternalServerError,
            "api.storage.read_write_failed",
            "Storage read/write failed",
        ),
    }
}

fn event_repository_error_to_response(error: AiotEventRepositoryError) -> HttpResponse {
    match error {
        AiotEventRepositoryError::PersistenceFailure => problem_response(
            HttpStatus::InternalServerError,
            "api.storage.read_failed",
            "Storage read failed",
        ),
    }
}

fn twin_repository_error_to_response(error: AiotDeviceTwinRepositoryError) -> HttpResponse {
    match error {
        AiotDeviceTwinRepositoryError::PersistenceFailure => problem_response(
            HttpStatus::InternalServerError,
            "api.storage.read_write_failed",
            "Storage read/write failed",
        ),
    }
}

fn firmware_repository_error_to_response(error: AiotFirmwareRepositoryError) -> HttpResponse {
    match error {
        AiotFirmwareRepositoryError::DuplicateArtifactId => problem_response(
            HttpStatus::Conflict,
            "api.firmware.artifact.duplicate_id",
            "Firmware artifact id already exists",
        ),
        AiotFirmwareRepositoryError::DuplicateRolloutId => problem_response(
            HttpStatus::Conflict,
            "api.firmware.rollout.duplicate_id",
            "Firmware rollout id already exists",
        ),
        AiotFirmwareRepositoryError::ArtifactNotFound => problem_response(
            HttpStatus::NotFound,
            "api.firmware.artifact.not_found",
            "Firmware artifact not found",
        ),
        AiotFirmwareRepositoryError::RolloutNotFound => problem_response(
            HttpStatus::NotFound,
            "api.firmware.rollout.not_found",
            "Firmware rollout not found",
        ),
        AiotFirmwareRepositoryError::InvalidReference => problem_response(
            HttpStatus::BadRequest,
            "api.firmware.artifact.invalid_reference",
            "Firmware artifact reference is invalid",
        ),
        AiotFirmwareRepositoryError::StorageFailure => problem_response(
            HttpStatus::InternalServerError,
            "api.storage.read_write_failed",
            "Storage read/write failed",
        ),
    }
}

fn catalog_repository_error_to_response(error: AiotCatalogRepositoryError) -> HttpResponse {
    match error {
        AiotCatalogRepositoryError::DuplicateProductId => problem_response(
            HttpStatus::Conflict,
            "api.product.duplicate_id",
            "Product id already exists",
        ),
        AiotCatalogRepositoryError::DuplicateHardwareProfileId => problem_response(
            HttpStatus::Conflict,
            "api.hardware_profile.duplicate_id",
            "Hardware profile id already exists",
        ),
        AiotCatalogRepositoryError::DuplicateProtocolProfileId => problem_response(
            HttpStatus::Conflict,
            "api.protocol_profile.duplicate_id",
            "Protocol profile id already exists",
        ),
        AiotCatalogRepositoryError::DuplicateCapabilityModelId => problem_response(
            HttpStatus::Conflict,
            "api.capability_model.duplicate_id",
            "Capability model id already exists",
        ),
        AiotCatalogRepositoryError::ProductNotFound => problem_response(
            HttpStatus::NotFound,
            "api.product.not_found",
            "Product not found",
        ),
        AiotCatalogRepositoryError::HardwareProfileNotFound => problem_response(
            HttpStatus::NotFound,
            "api.hardware_profile.not_found",
            "Hardware profile not found",
        ),
        AiotCatalogRepositoryError::ProtocolProfileNotFound => problem_response(
            HttpStatus::NotFound,
            "api.protocol_profile.not_found",
            "Protocol profile not found",
        ),
        AiotCatalogRepositoryError::CapabilityModelNotFound => problem_response(
            HttpStatus::NotFound,
            "api.capability_model.not_found",
            "Capability model not found",
        ),
        AiotCatalogRepositoryError::StorageFailure => problem_response(
            HttpStatus::InternalServerError,
            "api.storage.read_write_failed",
            "Storage read/write failed",
        ),
    }
}

fn protocol_adapter_item_json(runtime: &AiotRuntime) -> Vec<String> {
    runtime
        .protocol_routes()
        .iter()
        .map(|route| {
            let adapter = runtime.protocol_adapter_for(&route.protocol_id);
            let scope = adapter
                .map(|adapter| format!("{:?}", adapter.scope))
                .unwrap_or_default();
            let transports = adapter
                .map(|adapter| debug_array(adapter.transports.iter()))
                .unwrap_or_default();
            let codecs = adapter
                .map(|adapter| debug_array(adapter.codecs.iter()))
                .unwrap_or_default();
            let session_policies = adapter
                .map(|adapter| debug_array(adapter.session_policies.iter()))
                .unwrap_or_default();
            let security_modes = adapter
                .map(|adapter| string_array(adapter.security_modes.iter()))
                .unwrap_or_default();
            let hardware_families = adapter
                .map(|adapter| string_array(adapter.hardware_families.iter()))
                .unwrap_or_default();
            let runtime_profiles = adapter
                .map(|adapter| string_array(adapter.runtime_profiles.iter()))
                .unwrap_or_default();
            let firmware_profiles = adapter
                .map(|adapter| string_array(adapter.firmware_profiles.iter()))
                .unwrap_or_default();

            format!(
                r#"{{"path":"{}","protocolId":"{}","pluginId":"{}","scope":"{}","transport":"{:?}","transports":[{}],"codecs":[{}],"sessionPolicies":[{}],"securityModes":[{}],"hardwareFamilies":[{}],"runtimeProfiles":[{}],"firmwareProfiles":[{}],"kind":"{}"}}"#,
                route.path,
                route.protocol_id,
                route.plugin_id,
                scope,
                route.transport,
                transports,
                codecs,
                session_policies,
                security_modes,
                hardware_families,
                runtime_profiles,
                firmware_profiles,
                route_kind_name(route.kind)
            )
        })
        .collect()
}

fn runtime_capacity_data_json() -> String {
    let policy = sdkwork_aiot_service_host::AiotRuntimeCapacityPolicy::standard();

    format!(
        r#"{{"nodeId":"{}","maxConnectionsPerNode":"{}","maxSessionsPerTenant":"{}","maxInflightPerDevice":{},"sessionLeaseTtlSeconds":{},"sessionLeaseRenewSeconds":{},"outboxMaxAttempts":{},"deadLetterAfterAttempts":{},"backpressure":{{"warnLag":"{}","rejectLag":"{}","deadLetterLag":"{}"}},"orderedDeviceCommands":{},"idempotentIngest":{}}}"#,
        policy.node_id,
        policy.max_connections_per_node,
        policy.max_sessions_per_tenant,
        policy.max_inflight_per_device,
        policy.session_lease_ttl_seconds,
        policy.session_lease_renew_seconds,
        policy.outbox_max_attempts,
        policy.dead_letter_after_attempts,
        policy.outbox_warn_lag,
        policy.outbox_reject_lag,
        policy.outbox_dead_letter_lag,
        policy.enable_ordered_device_commands,
        policy.enable_idempotent_ingest
    )
}

fn standard_product_records() -> Vec<AiotProductRecord> {
    vec![
        AiotProductRecord {
            product_id: "9001".to_string(),
            display_name: "Xiaozhi Voice Assistant".to_string(),
            default_hardware_profile_id: "hw-esp32-s3".to_string(),
            default_protocol_profile_id: "proto-xiaozhi".to_string(),
            default_capability_model_id: "capmodel-xiaozhi-core".to_string(),
            status: "active".to_string(),
        },
        AiotProductRecord {
            product_id: "9002".to_string(),
            display_name: "Edge Audio Gateway".to_string(),
            default_hardware_profile_id: "hw-raspberry-pi-5".to_string(),
            default_protocol_profile_id: "proto-mqtt-standard".to_string(),
            default_capability_model_id: "capmodel-edge-gateway".to_string(),
            status: "active".to_string(),
        },
    ]
}

fn standard_hardware_profile_records() -> Vec<AiotHardwareProfileRecord> {
    vec![
        AiotHardwareProfileRecord {
            hardware_profile_id: "hw-esp32-s3".to_string(),
            chip_family: "esp32_s3".to_string(),
            hardware_classes: vec!["mcu".to_string()],
            runtime_profiles: vec!["esp_idf".to_string(), "freertos".to_string()],
            connectivity_profiles: vec!["wifi".to_string(), "ble".to_string()],
            security_profiles: vec![
                "secure_boot".to_string(),
                "flash_encryption".to_string(),
                "device_secret".to_string(),
            ],
            ota_profiles: vec!["xiaozhi_ota".to_string()],
            status: "active".to_string(),
        },
        AiotHardwareProfileRecord {
            hardware_profile_id: "hw-raspberry-pi-5".to_string(),
            chip_family: "bcm2712".to_string(),
            hardware_classes: vec!["linux_sbc".to_string(), "edge_gateway".to_string()],
            runtime_profiles: vec![
                "linux".to_string(),
                "docker".to_string(),
                "home_assistant".to_string(),
            ],
            connectivity_profiles: vec![
                "ethernet".to_string(),
                "wifi".to_string(),
                "zigbee_usb".to_string(),
            ],
            security_profiles: vec!["tpm".to_string(), "secure_boot".to_string()],
            ota_profiles: vec!["apt_container_image".to_string()],
            status: "active".to_string(),
        },
    ]
}

fn standard_protocol_profile_catalog() -> Vec<ProtocolProfile> {
    vec![
        ProtocolProfile::new("proto-xiaozhi", "xiaozhi.websocket")
            .allow_transport("websocket")
            .allow_transport("http")
            .allow_transport("mqtt")
            .allow_transport("udp")
            .allow_message_class("handshake")
            .allow_message_class("commandRequest")
            .allow_message_class("commandResult")
            .allow_message_class("mediaFrame")
            .allow_message_class("otaCheck")
            .allow_message_class("otaDeploy"),
        ProtocolProfile::new("proto-mqtt-standard", "mqtt.v5")
            .allow_transport("mqtt")
            .allow_message_class("telemetry")
            .allow_message_class("event")
            .allow_message_class("propertyReport")
            .allow_message_class("propertySet"),
    ]
}

fn standard_protocol_profile_records() -> Vec<AiotProtocolProfileRecord> {
    let protocol_catalog = standard_protocol_catalog();
    let mut records = Vec::new();
    for profile in standard_protocol_profile_catalog() {
        let scope = protocol_catalog
            .iter()
            .find(|entry| entry.protocol_id == profile.default_protocol_id)
            .map(|entry| protocol_scope_name(entry.scope).to_string())
            .unwrap_or_else(|| "StandardAdapter".to_string());
        let capability_bridges = protocol_catalog
            .iter()
            .find(|entry| entry.protocol_id == profile.default_protocol_id)
            .map(|entry| {
                entry
                    .capability_bridges
                    .iter()
                    .map(capability_bridge_name)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        records.push(AiotProtocolProfileRecord {
            protocol_profile_id: profile.profile_id,
            default_protocol_id: profile.default_protocol_id,
            scope,
            allowed_transports: profile.allowed_transports,
            allowed_message_classes: profile.allowed_message_classes,
            capability_bridges,
            status: "active".to_string(),
        });
    }
    records
}

fn standard_capability_models() -> Vec<AiotCapabilityModel> {
    vec![
        AiotCapabilityModel {
            capability_model_id: "capmodel-xiaozhi-core".to_string(),
            display_name: "Xiaozhi Core Capability Model".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec![
                CapabilityDefinition::new("audio.capture", CapabilityKind::Media)
                    .with_command("startCapture")
                    .with_command("stopCapture")
                    .with_event("audioChunk")
                    .with_protocol_mapping("xiaozhi.websocket", "listen")
                    .with_protocol_mapping("xiaozhi.mqtt_udp", "listen"),
                CapabilityDefinition::new("audio.playback", CapabilityKind::Media)
                    .with_command("speak")
                    .with_command("stop")
                    .with_event("playbackCompleted")
                    .with_protocol_mapping("xiaozhi.websocket", "tts")
                    .with_protocol_mapping("xiaozhi.mqtt_udp", "tts"),
                CapabilityDefinition::new("system.reboot", CapabilityKind::Command)
                    .with_command("rebootNow")
                    .with_event("rebooted")
                    .with_protocol_mapping("xiaozhi.websocket", "system.reboot"),
                CapabilityDefinition::new("assistant", CapabilityKind::Command)
                    .with_command("chat")
                    .with_event("chatCompleted"),
            ],
        },
        AiotCapabilityModel {
            capability_model_id: "capmodel-edge-gateway".to_string(),
            display_name: "Edge Gateway Capability Model".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec![
                CapabilityDefinition::new("gateway.topology", CapabilityKind::Event)
                    .with_event("topologyChanged")
                    .with_protocol_mapping("mqtt.v5", "gateway/topology")
                    .with_protocol_mapping("raspberrypi.linux_gateway", "gateway.topology"),
                CapabilityDefinition::new("device.shadow", CapabilityKind::Property)
                    .with_command("patchDesired")
                    .with_event("reportedChanged")
                    .with_protocol_mapping("mqtt.v5", "devices/{deviceId}/shadow"),
            ],
        },
    ]
}

fn standard_capability_model_records() -> Vec<AiotCapabilityModelRecord> {
    standard_capability_models()
        .into_iter()
        .map(|model| AiotCapabilityModelRecord {
            capability_model_id: model.capability_model_id,
            display_name: model.display_name,
            version: model.version,
            capabilities: model.capabilities,
            status: "active".to_string(),
        })
        .collect()
}

fn standard_product_collection_response(
    request: &HttpRequest,
    products: &[AiotProductRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = products
        .iter()
        .map(product_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn standard_product_response(
    request: &HttpRequest,
    status: HttpStatus,
    product: &AiotProductRecord,
) -> HttpResponse {
    standard_resource_response(request, status, product_resource_json(product))
}

fn standard_hardware_profile_collection_response(
    request: &HttpRequest,
    profiles: &[AiotHardwareProfileRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = profiles
        .iter()
        .map(hardware_profile_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn standard_hardware_profile_response(
    request: &HttpRequest,
    status: HttpStatus,
    profile: &AiotHardwareProfileRecord,
) -> HttpResponse {
    standard_resource_response(request, status, hardware_profile_resource_json(profile))
}

fn standard_protocol_profile_collection_response(
    request: &HttpRequest,
    profiles: &[AiotProtocolProfileRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = profiles
        .iter()
        .map(protocol_profile_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn standard_protocol_profile_response(
    request: &HttpRequest,
    status: HttpStatus,
    profile: &AiotProtocolProfileRecord,
) -> HttpResponse {
    standard_resource_response(request, status, protocol_profile_resource_json(profile))
}

fn standard_capability_model_collection_response(
    request: &HttpRequest,
    models: &[AiotCapabilityModelRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = models
        .iter()
        .map(capability_model_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn standard_capability_model_record_response(
    request: &HttpRequest,
    status: HttpStatus,
    model: &AiotCapabilityModelRecord,
) -> HttpResponse {
    standard_resource_response(request, status, capability_model_resource_json(model))
}

fn product_resource_json(product: &AiotProductRecord) -> String {
    format!(
        r#"{{"productId":"{}","displayName":"{}","defaultHardwareProfileId":"{}","defaultProtocolProfileId":"{}","defaultCapabilityModelId":"{}","status":"{}"}}"#,
        json_escape(&product.product_id),
        json_escape(&product.display_name),
        json_escape(&product.default_hardware_profile_id),
        json_escape(&product.default_protocol_profile_id),
        json_escape(&product.default_capability_model_id),
        json_escape(&product.status),
    )
}

fn hardware_profile_resource_json(profile: &AiotHardwareProfileRecord) -> String {
    format!(
        r#"{{"hardwareProfileId":"{}","chipFamily":"{}","hardwareClasses":[{}],"runtimeProfiles":[{}],"connectivityProfiles":[{}],"securityProfiles":[{}],"otaProfiles":[{}],"status":"{}"}}"#,
        json_escape(&profile.hardware_profile_id),
        json_escape(&profile.chip_family),
        string_array(profile.hardware_classes.iter()),
        string_array(profile.runtime_profiles.iter()),
        string_array(profile.connectivity_profiles.iter()),
        string_array(profile.security_profiles.iter()),
        string_array(profile.ota_profiles.iter()),
        json_escape(&profile.status),
    )
}

fn protocol_profile_resource_json(profile: &AiotProtocolProfileRecord) -> String {
    format!(
        r#"{{"protocolProfileId":"{}","defaultProtocolId":"{}","scope":"{}","allowedTransports":[{}],"allowedMessageClasses":[{}],"capabilityBridges":[{}],"status":"{}"}}"#,
        json_escape(&profile.protocol_profile_id),
        json_escape(&profile.default_protocol_id),
        json_escape(&profile.scope),
        string_array(profile.allowed_transports.iter()),
        string_array(profile.allowed_message_classes.iter()),
        string_array(profile.capability_bridges.iter()),
        json_escape(&profile.status),
    )
}

fn capability_model_resource_json(model: &AiotCapabilityModelRecord) -> String {
    let capabilities = model
        .capabilities
        .iter()
        .map(capability_definition_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"capabilityModelId":"{}","displayName":"{}","version":"{}","capabilities":[{}],"status":"{}"}}"#,
        json_escape(&model.capability_model_id),
        json_escape(&model.display_name),
        json_escape(&model.version),
        capabilities,
        json_escape(&model.status),
    )
}

fn capability_definition_resource_json(definition: &CapabilityDefinition) -> String {
    let mappings = definition
        .protocol_mappings
        .iter()
        .map(|(protocol_id, mapped_name)| {
            format!(
                r#"{{"protocolId":"{}","mappedName":"{}"}}"#,
                json_escape(protocol_id),
                json_escape(mapped_name)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"capabilityName":"{}","capabilityKind":"{}","commands":[{}],"events":[{}],"protocolMappings":[{}]}}"#,
        json_escape(&definition.name),
        capability_kind_name(definition.kind),
        string_array(definition.commands.iter()),
        string_array(definition.events.iter()),
        mappings,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotFirmwareArtifactRecord {
    artifact_id: String,
    tenant_id: i64,
    organization_id: i64,
    artifact_key: String,
    version: String,
    media_resource_id: String,
    resource_json: String,
    sha256: String,
    signature: Option<String>,
    target_chip_family: Option<String>,
    target_runtime_profile: Option<String>,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotFirmwareRolloutRecord {
    rollout_id: String,
    tenant_id: i64,
    organization_id: i64,
    artifact_id: String,
    target_policy_json: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotDeviceCapabilityRecord {
    capability_name: String,
    capability_kind: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotDeviceCredentialRecord {
    credential_id: String,
    tenant_id: i64,
    organization_id: i64,
    device_id: String,
    credential_type: String,
    status: String,
    expires_at: Option<String>,
    created_at: String,
    revoked_at: Option<String>,
    issued_secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotCapabilityModel {
    capability_model_id: String,
    display_name: String,
    version: String,
    capabilities: Vec<CapabilityDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotProductRecord {
    product_id: String,
    display_name: String,
    default_hardware_profile_id: String,
    default_protocol_profile_id: String,
    default_capability_model_id: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotHardwareProfileRecord {
    hardware_profile_id: String,
    chip_family: String,
    hardware_classes: Vec<String>,
    runtime_profiles: Vec<String>,
    connectivity_profiles: Vec<String>,
    security_profiles: Vec<String>,
    ota_profiles: Vec<String>,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotProtocolProfileRecord {
    protocol_profile_id: String,
    default_protocol_id: String,
    scope: String,
    allowed_transports: Vec<String>,
    allowed_message_classes: Vec<String>,
    capability_bridges: Vec<String>,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiotCapabilityModelRecord {
    capability_model_id: String,
    display_name: String,
    version: String,
    capabilities: Vec<CapabilityDefinition>,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AiotCatalogRepositoryError {
    DuplicateProductId,
    DuplicateHardwareProfileId,
    DuplicateProtocolProfileId,
    DuplicateCapabilityModelId,
    ProductNotFound,
    HardwareProfileNotFound,
    ProtocolProfileNotFound,
    CapabilityModelNotFound,
    StorageFailure,
}

#[derive(Debug, Clone)]
struct AiotProductCreatePayload {
    product_id: String,
    display_name: String,
    default_hardware_profile_id: String,
    default_protocol_profile_id: String,
    default_capability_model_id: String,
}

#[derive(Debug, Clone, Default)]
struct AiotProductUpdatePayload {
    display_name: Option<String>,
    default_hardware_profile_id: Option<String>,
    default_protocol_profile_id: Option<String>,
    default_capability_model_id: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Clone)]
struct AiotHardwareProfileCreatePayload {
    hardware_profile_id: String,
    chip_family: String,
    hardware_classes: Vec<String>,
    runtime_profiles: Vec<String>,
    connectivity_profiles: Vec<String>,
    security_profiles: Vec<String>,
    ota_profiles: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct AiotHardwareProfileUpdatePayload {
    chip_family: Option<String>,
    hardware_classes: Option<Vec<String>>,
    runtime_profiles: Option<Vec<String>>,
    connectivity_profiles: Option<Vec<String>>,
    security_profiles: Option<Vec<String>>,
    ota_profiles: Option<Vec<String>>,
    status: Option<String>,
}

#[derive(Debug, Clone)]
struct AiotProtocolProfileCreatePayload {
    protocol_profile_id: String,
    default_protocol_id: String,
    scope: String,
    allowed_transports: Vec<String>,
    allowed_message_classes: Vec<String>,
    capability_bridges: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct AiotProtocolProfileUpdatePayload {
    default_protocol_id: Option<String>,
    scope: Option<String>,
    allowed_transports: Option<Vec<String>>,
    allowed_message_classes: Option<Vec<String>>,
    capability_bridges: Option<Vec<String>>,
    status: Option<String>,
}

#[derive(Debug, Clone)]
struct AiotCapabilityModelCreatePayload {
    capability_model_id: String,
    display_name: String,
    version: String,
    capabilities: Vec<CapabilityDefinition>,
}

#[derive(Debug, Clone, Default)]
struct AiotCapabilityModelUpdatePayload {
    display_name: Option<String>,
    version: Option<String>,
    capabilities: Option<Vec<CapabilityDefinition>>,
    status: Option<String>,
}

#[derive(Debug, Clone)]
struct AiotFirmwareArtifactCreatePayload {
    artifact_key: String,
    version: String,
    resource_json: String,
    media_resource_id: String,
    object_blob_id: Option<String>,
    sha256: String,
    signature: Option<String>,
    target_chip_family: Option<String>,
    target_runtime_profile: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct AiotFirmwareArtifactUpdatePayload {
    artifact_key: Option<String>,
    version: Option<String>,
    resource_json: Option<String>,
    media_resource_id: Option<String>,
    object_blob_id: Option<String>,
    sha256: Option<String>,
    signature: Option<String>,
    target_chip_family: Option<String>,
    target_runtime_profile: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Clone)]
struct AiotFirmwareRolloutCreatePayload {
    artifact_id: String,
    target_policy_json: String,
}

#[derive(Debug, Clone, Default)]
struct AiotFirmwareRolloutUpdatePayload {
    target_policy_json: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AiotFirmwareRepositoryError {
    DuplicateArtifactId,
    DuplicateRolloutId,
    ArtifactNotFound,
    RolloutNotFound,
    InvalidReference,
    StorageFailure,
}

#[derive(Debug, Default)]
struct InMemoryAiotFirmwareRepositoryState {
    next_artifact_id: u64,
    next_rollout_id: u64,
    next_deployment_id: u64,
    artifacts: BTreeMap<String, AiotFirmwareArtifactRecord>,
    rollouts: BTreeMap<String, AiotFirmwareRolloutRecord>,
    deployments: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotFirmwareRepository {
    state: Arc<Mutex<InMemoryAiotFirmwareRepositoryState>>,
}

#[derive(Debug, Default)]
struct InMemoryAiotCredentialRepositoryState {
    next_credential_id: u64,
    credentials: BTreeMap<String, AiotDeviceCredentialRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotCredentialRepository {
    state: Arc<Mutex<InMemoryAiotCredentialRepositoryState>>,
}

#[derive(Debug, Default)]
struct InMemoryAiotCatalogRepositoryState {
    products: BTreeMap<String, AiotProductRecord>,
    hardware_profiles: BTreeMap<String, AiotHardwareProfileRecord>,
    protocol_profiles: BTreeMap<String, AiotProtocolProfileRecord>,
    capability_models: BTreeMap<String, AiotCapabilityModelRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotCatalogRepository {
    state: Arc<Mutex<InMemoryAiotCatalogRepositoryState>>,
}

impl InMemoryAiotFirmwareRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn create_artifact(
        &self,
        association: AiotStorageAssociation,
        payload: AiotFirmwareArtifactCreatePayload,
    ) -> Result<AiotFirmwareArtifactRecord, AiotFirmwareRepositoryError> {
        let mut state = self.state.lock().expect("in-memory firmware repo poisoned");
        let artifact_id = format!("firmware-artifact-{:04}", state.next_artifact_id + 1);
        let key = scoped_firmware_artifact_key(&association, &artifact_id);
        if state.artifacts.contains_key(&key) {
            return Err(AiotFirmwareRepositoryError::DuplicateArtifactId);
        }
        let resource_json = if let Some(object_blob_id) = payload.object_blob_id.as_deref() {
            apply_media_object_blob_id(&payload.resource_json, object_blob_id)
                .unwrap_or_else(|_| payload.resource_json.clone())
        } else {
            payload.resource_json.clone()
        };
        state.next_artifact_id += 1;
        let record = AiotFirmwareArtifactRecord {
            artifact_id,
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
        };
        state.artifacts.insert(key, record.clone());
        Ok(record)
    }

    fn list_artifacts(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotFirmwareArtifactRecord> {
        self.state
            .lock()
            .expect("in-memory firmware repo poisoned")
            .artifacts
            .values()
            .filter(|artifact| {
                artifact.tenant_id == association.tenant_id
                    && artifact.organization_id == association.organization_id
            })
            .cloned()
            .collect()
    }

    fn get_artifact(
        &self,
        association: &AiotStorageAssociation,
        artifact_id: &str,
    ) -> Option<AiotFirmwareArtifactRecord> {
        self.state
            .lock()
            .expect("in-memory firmware repo poisoned")
            .artifacts
            .get(&scoped_firmware_artifact_key(association, artifact_id))
            .cloned()
    }

    fn update_artifact(
        &self,
        association: AiotStorageAssociation,
        artifact_id: &str,
        payload: AiotFirmwareArtifactUpdatePayload,
    ) -> Result<AiotFirmwareArtifactRecord, AiotFirmwareRepositoryError> {
        let mut state = self.state.lock().expect("in-memory firmware repo poisoned");
        let key = scoped_firmware_artifact_key(&association, artifact_id);
        let Some(record) = state.artifacts.get_mut(&key) else {
            return Err(AiotFirmwareRepositoryError::ArtifactNotFound);
        };
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
                apply_media_object_blob_id(&record.resource_json, &object_blob_id)
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
        Ok(record.clone())
    }

    fn delete_artifact(
        &self,
        association: &AiotStorageAssociation,
        artifact_id: &str,
    ) -> Result<(), AiotFirmwareRepositoryError> {
        let mut state = self.state.lock().expect("in-memory firmware repo poisoned");
        let key = scoped_firmware_artifact_key(association, artifact_id);
        if state.artifacts.remove(&key).is_some() {
            Ok(())
        } else {
            Err(AiotFirmwareRepositoryError::ArtifactNotFound)
        }
    }

    fn create_rollout(
        &self,
        association: AiotStorageAssociation,
        payload: AiotFirmwareRolloutCreatePayload,
        target_device_ids: &[String],
    ) -> Result<AiotFirmwareRolloutRecord, AiotFirmwareRepositoryError> {
        let mut state = self.state.lock().expect("in-memory firmware repo poisoned");
        let artifact_key = scoped_firmware_artifact_key(&association, &payload.artifact_id);
        if !state.artifacts.contains_key(&artifact_key) {
            return Err(AiotFirmwareRepositoryError::InvalidReference);
        }
        let rollout_id = format!("firmware-rollout-{:04}", state.next_rollout_id + 1);
        let key = scoped_firmware_rollout_key(&association, &rollout_id);
        if state.rollouts.contains_key(&key) {
            return Err(AiotFirmwareRepositoryError::DuplicateRolloutId);
        }
        state.next_rollout_id += 1;
        let record = AiotFirmwareRolloutRecord {
            rollout_id: rollout_id.clone(),
            tenant_id: association.tenant_id,
            organization_id: association.organization_id,
            artifact_id: payload.artifact_id.clone(),
            target_policy_json: payload.target_policy_json.clone(),
            status: "accepted".to_string(),
        };
        state.rollouts.insert(key, record.clone());

        let force = rollout_force_from_policy(&payload.target_policy_json);
        for device_id in target_device_ids {
            state.next_deployment_id += 1;
            let deployment_id = format!("firmware-deployment-{:04}", state.next_deployment_id);
            let deployment_key = scoped_firmware_deployment_key(&association, &deployment_id);
            let deployment_json = firmware_deployment_payload_json(
                &deployment_id,
                &association,
                &rollout_id,
                &payload.artifact_id,
                device_id,
                force,
            );
            state.deployments.insert(deployment_key, deployment_json);
        }

        Ok(record)
    }

    fn list_rollouts(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotFirmwareRolloutRecord> {
        self.state
            .lock()
            .expect("in-memory firmware repo poisoned")
            .rollouts
            .values()
            .filter(|rollout| {
                rollout.tenant_id == association.tenant_id
                    && rollout.organization_id == association.organization_id
            })
            .cloned()
            .collect()
    }

    fn get_rollout(
        &self,
        association: &AiotStorageAssociation,
        rollout_id: &str,
    ) -> Option<AiotFirmwareRolloutRecord> {
        self.state
            .lock()
            .expect("in-memory firmware repo poisoned")
            .rollouts
            .get(&scoped_firmware_rollout_key(association, rollout_id))
            .cloned()
    }

    fn update_rollout(
        &self,
        association: AiotStorageAssociation,
        rollout_id: &str,
        payload: AiotFirmwareRolloutUpdatePayload,
    ) -> Result<AiotFirmwareRolloutRecord, AiotFirmwareRepositoryError> {
        let mut state = self.state.lock().expect("in-memory firmware repo poisoned");
        let key = scoped_firmware_rollout_key(&association, rollout_id);
        let Some(record) = state.rollouts.get_mut(&key) else {
            return Err(AiotFirmwareRepositoryError::RolloutNotFound);
        };
        if let Some(target_policy_json) = payload.target_policy_json {
            record.target_policy_json = target_policy_json;
        }
        if let Some(status) = payload.status {
            record.status = status;
        }
        Ok(record.clone())
    }

    fn delete_rollout(
        &self,
        association: &AiotStorageAssociation,
        rollout_id: &str,
    ) -> Result<(), AiotFirmwareRepositoryError> {
        let mut state = self.state.lock().expect("in-memory firmware repo poisoned");
        let key = scoped_firmware_rollout_key(association, rollout_id);
        if state.rollouts.remove(&key).is_some() {
            Ok(())
        } else {
            Err(AiotFirmwareRepositoryError::RolloutNotFound)
        }
    }
}

impl InMemoryAiotCredentialRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn create_credential(
        &self,
        association: AiotStorageAssociation,
        command: AiotCredentialCreateCommand,
    ) -> Result<AiotDeviceCredentialRecord, HttpResponse> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory credential repo poisoned");
        state.next_credential_id += 1;
        let credential_id = format!("credential-{:04}", state.next_credential_id);
        let key = scoped_device_credential_key(&association, &command.device_id, &credential_id);
        let record = AiotDeviceCredentialRecord {
            credential_id,
            tenant_id: association.tenant_id,
            organization_id: association.organization_id,
            device_id: command.device_id,
            credential_type: command.credential_type,
            status: "active".to_string(),
            expires_at: command.expires_at,
            created_at: "2026-06-01T00:00:00Z".to_string(),
            revoked_at: None,
            issued_secret: None,
        };
        state.credentials.insert(key, record.clone());
        Ok(record)
    }

    fn list_credentials(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceCredentialRecord>, HttpResponse> {
        let items = self
            .state
            .lock()
            .expect("in-memory credential repo poisoned")
            .credentials
            .values()
            .filter(|credential| {
                credential.tenant_id == association.tenant_id
                    && credential.organization_id == association.organization_id
                    && credential.device_id == device_id
            })
            .cloned()
            .collect::<Vec<_>>();
        Ok(paginate_vec(items, params))
    }

    fn get_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Option<AiotDeviceCredentialRecord> {
        self.state
            .lock()
            .expect("in-memory credential repo poisoned")
            .credentials
            .get(&scoped_device_credential_key(
                association,
                device_id,
                credential_id,
            ))
            .cloned()
    }

    fn delete_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Result<(), HttpResponse> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory credential repo poisoned");
        let key = scoped_device_credential_key(association, device_id, credential_id);
        let Some(record) = state.credentials.get_mut(&key) else {
            return Err(credential_not_found_response(credential_id));
        };
        if record.status != "revoked" {
            record.status = "revoked".to_string();
            record.revoked_at = Some("2026-06-01T00:00:00Z".to_string());
        }
        Ok(())
    }
}

impl AiotCredentialRepository for InMemoryAiotCredentialRepository {
    fn create_credential(
        &self,
        association: AiotStorageAssociation,
        command: AiotCredentialCreateCommand,
    ) -> Result<AiotDeviceCredentialRecord, HttpResponse> {
        InMemoryAiotCredentialRepository::create_credential(self, association, command)
    }

    fn list_credentials(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceCredentialRecord>, HttpResponse> {
        InMemoryAiotCredentialRepository::list_credentials(self, association, device_id, params)
    }

    fn get_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Option<AiotDeviceCredentialRecord> {
        InMemoryAiotCredentialRepository::get_credential(
            self,
            association,
            device_id,
            credential_id,
        )
    }

    fn delete_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Result<(), HttpResponse> {
        InMemoryAiotCredentialRepository::delete_credential(
            self,
            association,
            device_id,
            credential_id,
        )
    }
}

impl InMemoryAiotCatalogRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn create_product(
        &self,
        association: AiotStorageAssociation,
        payload: AiotProductCreatePayload,
    ) -> Result<AiotProductRecord, AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(&association, &payload.product_id);
        if state.products.contains_key(&key) {
            return Err(AiotCatalogRepositoryError::DuplicateProductId);
        }
        let record = AiotProductRecord {
            product_id: payload.product_id,
            display_name: payload.display_name,
            default_hardware_profile_id: payload.default_hardware_profile_id,
            default_protocol_profile_id: payload.default_protocol_profile_id,
            default_capability_model_id: payload.default_capability_model_id,
            status: "active".to_string(),
        };
        state.products.insert(key, record.clone());
        Ok(record)
    }

    fn get_product(
        &self,
        association: &AiotStorageAssociation,
        product_id: &str,
    ) -> Option<AiotProductRecord> {
        self.state
            .lock()
            .expect("in-memory catalog repo poisoned")
            .products
            .get(&scoped_catalog_key(association, product_id))
            .cloned()
    }

    fn list_products(&self, association: &AiotStorageAssociation) -> Vec<AiotProductRecord> {
        let prefix = scoped_catalog_prefix(association);
        self.state
            .lock()
            .expect("in-memory catalog repo poisoned")
            .products
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, record)| record.clone())
            .collect()
    }

    fn update_product(
        &self,
        association: AiotStorageAssociation,
        product_id: &str,
        payload: AiotProductUpdatePayload,
    ) -> Result<AiotProductRecord, AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(&association, product_id);
        let Some(record) = state.products.get_mut(&key) else {
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
        Ok(record.clone())
    }

    fn delete_product(
        &self,
        association: &AiotStorageAssociation,
        product_id: &str,
    ) -> Result<(), AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(association, product_id);
        if state.products.remove(&key).is_some() {
            Ok(())
        } else {
            Err(AiotCatalogRepositoryError::ProductNotFound)
        }
    }

    fn create_hardware_profile(
        &self,
        association: AiotStorageAssociation,
        payload: AiotHardwareProfileCreatePayload,
    ) -> Result<AiotHardwareProfileRecord, AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(&association, &payload.hardware_profile_id);
        if state.hardware_profiles.contains_key(&key) {
            return Err(AiotCatalogRepositoryError::DuplicateHardwareProfileId);
        }
        let record = AiotHardwareProfileRecord {
            hardware_profile_id: payload.hardware_profile_id,
            chip_family: payload.chip_family,
            hardware_classes: payload.hardware_classes,
            runtime_profiles: payload.runtime_profiles,
            connectivity_profiles: payload.connectivity_profiles,
            security_profiles: payload.security_profiles,
            ota_profiles: payload.ota_profiles,
            status: "active".to_string(),
        };
        state.hardware_profiles.insert(key, record.clone());
        Ok(record)
    }

    fn get_hardware_profile(
        &self,
        association: &AiotStorageAssociation,
        hardware_profile_id: &str,
    ) -> Option<AiotHardwareProfileRecord> {
        self.state
            .lock()
            .expect("in-memory catalog repo poisoned")
            .hardware_profiles
            .get(&scoped_catalog_key(association, hardware_profile_id))
            .cloned()
    }

    fn list_hardware_profiles(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotHardwareProfileRecord> {
        let prefix = scoped_catalog_prefix(association);
        self.state
            .lock()
            .expect("in-memory catalog repo poisoned")
            .hardware_profiles
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, record)| record.clone())
            .collect()
    }

    fn update_hardware_profile(
        &self,
        association: AiotStorageAssociation,
        hardware_profile_id: &str,
        payload: AiotHardwareProfileUpdatePayload,
    ) -> Result<AiotHardwareProfileRecord, AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(&association, hardware_profile_id);
        let Some(record) = state.hardware_profiles.get_mut(&key) else {
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
        Ok(record.clone())
    }

    fn delete_hardware_profile(
        &self,
        association: &AiotStorageAssociation,
        hardware_profile_id: &str,
    ) -> Result<(), AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(association, hardware_profile_id);
        if state.hardware_profiles.remove(&key).is_some() {
            Ok(())
        } else {
            Err(AiotCatalogRepositoryError::HardwareProfileNotFound)
        }
    }

    fn create_protocol_profile(
        &self,
        association: AiotStorageAssociation,
        payload: AiotProtocolProfileCreatePayload,
    ) -> Result<AiotProtocolProfileRecord, AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(&association, &payload.protocol_profile_id);
        if state.protocol_profiles.contains_key(&key) {
            return Err(AiotCatalogRepositoryError::DuplicateProtocolProfileId);
        }
        let record = AiotProtocolProfileRecord {
            protocol_profile_id: payload.protocol_profile_id,
            default_protocol_id: payload.default_protocol_id,
            scope: payload.scope,
            allowed_transports: payload.allowed_transports,
            allowed_message_classes: payload.allowed_message_classes,
            capability_bridges: payload.capability_bridges,
            status: "active".to_string(),
        };
        state.protocol_profiles.insert(key, record.clone());
        Ok(record)
    }

    fn get_protocol_profile(
        &self,
        association: &AiotStorageAssociation,
        protocol_profile_id: &str,
    ) -> Option<AiotProtocolProfileRecord> {
        self.state
            .lock()
            .expect("in-memory catalog repo poisoned")
            .protocol_profiles
            .get(&scoped_catalog_key(association, protocol_profile_id))
            .cloned()
    }

    fn list_protocol_profiles(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotProtocolProfileRecord> {
        let prefix = scoped_catalog_prefix(association);
        self.state
            .lock()
            .expect("in-memory catalog repo poisoned")
            .protocol_profiles
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, record)| record.clone())
            .collect()
    }

    fn update_protocol_profile(
        &self,
        association: AiotStorageAssociation,
        protocol_profile_id: &str,
        payload: AiotProtocolProfileUpdatePayload,
    ) -> Result<AiotProtocolProfileRecord, AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(&association, protocol_profile_id);
        let Some(record) = state.protocol_profiles.get_mut(&key) else {
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
        Ok(record.clone())
    }

    fn delete_protocol_profile(
        &self,
        association: &AiotStorageAssociation,
        protocol_profile_id: &str,
    ) -> Result<(), AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(association, protocol_profile_id);
        if state.protocol_profiles.remove(&key).is_some() {
            Ok(())
        } else {
            Err(AiotCatalogRepositoryError::ProtocolProfileNotFound)
        }
    }

    fn create_capability_model(
        &self,
        association: AiotStorageAssociation,
        payload: AiotCapabilityModelCreatePayload,
    ) -> Result<AiotCapabilityModelRecord, AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(&association, &payload.capability_model_id);
        if state.capability_models.contains_key(&key) {
            return Err(AiotCatalogRepositoryError::DuplicateCapabilityModelId);
        }
        let record = AiotCapabilityModelRecord {
            capability_model_id: payload.capability_model_id,
            display_name: payload.display_name,
            version: payload.version,
            capabilities: payload.capabilities,
            status: "active".to_string(),
        };
        state.capability_models.insert(key, record.clone());
        Ok(record)
    }

    fn get_seed_capability_model(
        &self,
        capability_model_id: &str,
    ) -> Option<AiotCapabilityModelRecord> {
        standard_capability_model_records()
            .into_iter()
            .find(|record| record.capability_model_id == capability_model_id)
    }

    fn get_capability_model(
        &self,
        association: &AiotStorageAssociation,
        capability_model_id: &str,
    ) -> Option<AiotCapabilityModelRecord> {
        self.state
            .lock()
            .expect("in-memory catalog repo poisoned")
            .capability_models
            .get(&scoped_catalog_key(association, capability_model_id))
            .cloned()
    }

    fn list_capability_models(
        &self,
        association: &AiotStorageAssociation,
    ) -> Vec<AiotCapabilityModelRecord> {
        let prefix = scoped_catalog_prefix(association);
        self.state
            .lock()
            .expect("in-memory catalog repo poisoned")
            .capability_models
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, record)| record.clone())
            .collect()
    }

    fn update_capability_model(
        &self,
        association: AiotStorageAssociation,
        capability_model_id: &str,
        payload: AiotCapabilityModelUpdatePayload,
    ) -> Result<AiotCapabilityModelRecord, AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(&association, capability_model_id);
        let Some(record) = state.capability_models.get_mut(&key) else {
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
        Ok(record.clone())
    }

    fn delete_capability_model(
        &self,
        association: &AiotStorageAssociation,
        capability_model_id: &str,
    ) -> Result<(), AiotCatalogRepositoryError> {
        let mut state = self.state.lock().expect("in-memory catalog repo poisoned");
        let key = scoped_catalog_key(association, capability_model_id);
        if state.capability_models.remove(&key).is_some() {
            Ok(())
        } else {
            Err(AiotCatalogRepositoryError::CapabilityModelNotFound)
        }
    }
}

fn scoped_firmware_artifact_key(association: &AiotStorageAssociation, artifact_id: &str) -> String {
    format!(
        "{}:{}:{}",
        association.tenant_id, association.organization_id, artifact_id
    )
}

fn scoped_firmware_rollout_key(association: &AiotStorageAssociation, rollout_id: &str) -> String {
    format!(
        "{}:{}:{}",
        association.tenant_id, association.organization_id, rollout_id
    )
}

fn scoped_firmware_deployment_key(
    association: &AiotStorageAssociation,
    deployment_id: &str,
) -> String {
    format!(
        "{}:{}:{}",
        association.tenant_id, association.organization_id, deployment_id
    )
}

fn scoped_device_credential_key(
    association: &AiotStorageAssociation,
    device_id: &str,
    credential_id: &str,
) -> String {
    format!(
        "{}:{}:{}:{}",
        association.tenant_id, association.organization_id, device_id, credential_id
    )
}

fn scoped_catalog_key(association: &AiotStorageAssociation, catalog_id: &str) -> String {
    format!("{}{}", scoped_catalog_prefix(association), catalog_id)
}

fn scoped_catalog_prefix(association: &AiotStorageAssociation) -> String {
    format!("{}:{}:", association.tenant_id, association.organization_id)
}

#[derive(Debug, Clone)]
struct AiotDeviceCreatePayload {
    device_id: String,
    display_name: String,
    product_id: String,
    client_id: Option<String>,
    chip_family: Option<String>,
}

#[derive(Debug, Clone)]
struct AiotCredentialCreatePayload {
    credential_type: String,
    expires_at: Option<String>,
}

#[derive(Debug, Clone)]
struct AiotCredentialCreateCommand {
    device_id: String,
    credential_type: String,
    expires_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct AiotTwinUpdatePayload {
    desired: BTreeMap<String, String>,
    reported: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
struct AiotDeviceUpdatePayload {
    display_name: Option<String>,
    status: Option<String>,
    metadata_json: Option<String>,
}

fn required_json_object_body(
    request: &HttpRequest,
) -> Result<JsonMap<String, JsonValue>, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.body.required",
            "Request body is required",
        ));
    }

    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    body.as_object().cloned().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })
}

fn optional_json_object_body(
    request: &HttpRequest,
) -> Result<JsonMap<String, JsonValue>, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(JsonMap::new());
    }

    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    body.as_object().cloned().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })
}

fn product_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotProductCreatePayload, HttpResponse> {
    let obj = required_json_object_body(request)?;
    Ok(AiotProductCreatePayload {
        product_id: required_json_string_field(&obj, "productId")?,
        display_name: required_json_string_field(&obj, "displayName")?,
        default_hardware_profile_id: required_json_string_field(&obj, "defaultHardwareProfileId")?,
        default_protocol_profile_id: required_json_string_field(&obj, "defaultProtocolProfileId")?,
        default_capability_model_id: required_json_string_field(&obj, "defaultCapabilityModelId")?,
    })
}

fn product_update_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotProductUpdatePayload, HttpResponse> {
    let obj = optional_json_object_body(request)?;
    Ok(AiotProductUpdatePayload {
        display_name: optional_json_string_field(&obj, "displayName"),
        default_hardware_profile_id: optional_json_string_field(&obj, "defaultHardwareProfileId"),
        default_protocol_profile_id: optional_json_string_field(&obj, "defaultProtocolProfileId"),
        default_capability_model_id: optional_json_string_field(&obj, "defaultCapabilityModelId"),
        status: optional_json_string_field(&obj, "status"),
    })
}

fn hardware_profile_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotHardwareProfileCreatePayload, HttpResponse> {
    let obj = required_json_object_body(request)?;
    Ok(AiotHardwareProfileCreatePayload {
        hardware_profile_id: required_json_string_field(&obj, "hardwareProfileId")?,
        chip_family: required_json_string_field(&obj, "chipFamily")?,
        hardware_classes: optional_json_string_array_field(&obj, "hardwareClasses")?
            .unwrap_or_default(),
        runtime_profiles: optional_json_string_array_field(&obj, "runtimeProfiles")?
            .unwrap_or_default(),
        connectivity_profiles: optional_json_string_array_field(&obj, "connectivityProfiles")?
            .unwrap_or_default(),
        security_profiles: optional_json_string_array_field(&obj, "securityProfiles")?
            .unwrap_or_default(),
        ota_profiles: optional_json_string_array_field(&obj, "otaProfiles")?.unwrap_or_default(),
    })
}

fn hardware_profile_update_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotHardwareProfileUpdatePayload, HttpResponse> {
    let obj = optional_json_object_body(request)?;
    Ok(AiotHardwareProfileUpdatePayload {
        chip_family: optional_json_string_field(&obj, "chipFamily"),
        hardware_classes: optional_json_string_array_field(&obj, "hardwareClasses")?,
        runtime_profiles: optional_json_string_array_field(&obj, "runtimeProfiles")?,
        connectivity_profiles: optional_json_string_array_field(&obj, "connectivityProfiles")?,
        security_profiles: optional_json_string_array_field(&obj, "securityProfiles")?,
        ota_profiles: optional_json_string_array_field(&obj, "otaProfiles")?,
        status: optional_json_string_field(&obj, "status"),
    })
}

fn protocol_profile_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotProtocolProfileCreatePayload, HttpResponse> {
    let obj = required_json_object_body(request)?;
    Ok(AiotProtocolProfileCreatePayload {
        protocol_profile_id: required_json_string_field(&obj, "protocolProfileId")?,
        default_protocol_id: required_json_string_field(&obj, "defaultProtocolId")?,
        scope: optional_json_string_field(&obj, "scope")
            .unwrap_or_else(|| "StandardAdapter".to_string()),
        allowed_transports: optional_json_string_array_field(&obj, "allowedTransports")?
            .unwrap_or_default(),
        allowed_message_classes: optional_json_string_array_field(&obj, "allowedMessageClasses")?
            .unwrap_or_default(),
        capability_bridges: optional_json_string_array_field(&obj, "capabilityBridges")?
            .unwrap_or_default(),
    })
}

fn protocol_profile_update_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotProtocolProfileUpdatePayload, HttpResponse> {
    let obj = optional_json_object_body(request)?;
    Ok(AiotProtocolProfileUpdatePayload {
        default_protocol_id: optional_json_string_field(&obj, "defaultProtocolId"),
        scope: optional_json_string_field(&obj, "scope"),
        allowed_transports: optional_json_string_array_field(&obj, "allowedTransports")?,
        allowed_message_classes: optional_json_string_array_field(&obj, "allowedMessageClasses")?,
        capability_bridges: optional_json_string_array_field(&obj, "capabilityBridges")?,
        status: optional_json_string_field(&obj, "status"),
    })
}

fn capability_model_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotCapabilityModelCreatePayload, HttpResponse> {
    let obj = required_json_object_body(request)?;
    Ok(AiotCapabilityModelCreatePayload {
        capability_model_id: required_json_string_field(&obj, "capabilityModelId")?,
        display_name: required_json_string_field(&obj, "displayName")?,
        version: required_json_string_field(&obj, "version")?,
        capabilities: optional_json_capability_definitions_field(&obj, "capabilities")?
            .unwrap_or_default(),
    })
}

fn capability_model_update_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotCapabilityModelUpdatePayload, HttpResponse> {
    let obj = optional_json_object_body(request)?;
    Ok(AiotCapabilityModelUpdatePayload {
        display_name: optional_json_string_field(&obj, "displayName"),
        version: optional_json_string_field(&obj, "version"),
        capabilities: optional_json_capability_definitions_field(&obj, "capabilities")?,
        status: optional_json_string_field(&obj, "status"),
    })
}

fn device_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotDeviceCreatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.body.required",
            "Request body is required",
        ));
    }

    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;

    let device_id = required_json_string_field(obj, "deviceId")?;
    let display_name = required_json_string_field(obj, "displayName")?;
    let product_id = required_json_int64_string_field(obj, "productId")?;

    Ok(AiotDeviceCreatePayload {
        device_id,
        display_name,
        product_id,
        client_id: optional_json_string_field(obj, "clientId"),
        chip_family: optional_json_string_field(obj, "chipFamily"),
    })
}

fn firmware_artifact_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotFirmwareArtifactCreatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.body.required",
            "Request body is required",
        ));
    }
    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;

    let artifact_key = required_json_string_field(obj, "artifactKey")?;
    let version = required_json_string_field(obj, "version")?;
    let sha256 = required_json_string_field(obj, "sha256")?;
    let resource_value = obj.get("resource").ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            "Field resource is required",
        )
    })?;
    if !resource_value.is_object() {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            "Field resource must be a JSON object",
        ));
    }
    let resource_obj = resource_value.as_object().expect("resource object checked");
    let media_resource_id = json_object_string_field(resource_obj, "id")
        .map(str::to_string)
        .or_else(|| optional_json_string_field(obj, "mediaResourceId"))
        .ok_or_else(|| {
            problem_response(
                HttpStatus::BadRequest,
                "api.request.invalid_field",
                "Field resource.id or mediaResourceId is required",
            )
        })?;
    let object_blob_id = json_object_string_field(resource_obj, "objectBlobId")
        .map(str::to_string)
        .or_else(|| optional_json_string_field(obj, "objectBlobId"));

    Ok(AiotFirmwareArtifactCreatePayload {
        artifact_key,
        version,
        resource_json: resource_value.to_string(),
        media_resource_id,
        object_blob_id,
        sha256,
        signature: optional_json_string_field(obj, "signature"),
        target_chip_family: optional_json_string_field(obj, "targetChipFamily"),
        target_runtime_profile: optional_json_string_field(obj, "targetRuntimeProfile"),
    })
}

fn firmware_artifact_update_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotFirmwareArtifactUpdatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(AiotFirmwareArtifactUpdatePayload::default());
    }
    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;

    let mut payload = AiotFirmwareArtifactUpdatePayload {
        artifact_key: optional_json_string_field(obj, "artifactKey"),
        version: optional_json_string_field(obj, "version"),
        resource_json: obj.get("resource").map(JsonValue::to_string),
        media_resource_id: optional_json_string_field(obj, "mediaResourceId"),
        object_blob_id: optional_json_string_field(obj, "objectBlobId"),
        sha256: optional_json_string_field(obj, "sha256"),
        signature: optional_json_string_field(obj, "signature"),
        target_chip_family: optional_json_string_field(obj, "targetChipFamily"),
        target_runtime_profile: optional_json_string_field(obj, "targetRuntimeProfile"),
        status: optional_json_string_field(obj, "status"),
    };

    if let Some(resource_json) = payload.resource_json.as_deref() {
        let parsed: JsonValue = serde_json::from_str(resource_json).map_err(|_| {
            problem_response(
                HttpStatus::BadRequest,
                "api.request.invalid_field",
                "Field resource must be a valid JSON object",
            )
        })?;
        if !parsed.is_object() {
            return Err(problem_response(
                HttpStatus::BadRequest,
                "api.request.invalid_field",
                "Field resource must be a JSON object",
            ));
        }
        if payload.media_resource_id.is_none() {
            payload.media_resource_id = parsed
                .as_object()
                .and_then(|obj| json_object_string_field(obj, "id"))
                .map(str::to_string);
        }
        if payload.object_blob_id.is_none() {
            payload.object_blob_id = parsed
                .as_object()
                .and_then(|obj| json_object_string_field(obj, "objectBlobId"))
                .map(str::to_string);
        }
    }

    Ok(payload)
}

fn firmware_rollout_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotFirmwareRolloutCreatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.body.required",
            "Request body is required",
        ));
    }
    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;
    let artifact_id = required_json_string_field(obj, "artifactId")?;
    let target_policy_json = obj
        .get("targetPolicy")
        .map(JsonValue::to_string)
        .ok_or_else(|| {
            problem_response(
                HttpStatus::BadRequest,
                "api.request.invalid_field",
                "Field targetPolicy is required",
            )
        })?;
    Ok(AiotFirmwareRolloutCreatePayload {
        artifact_id,
        target_policy_json,
    })
}

fn firmware_rollout_update_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotFirmwareRolloutUpdatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(AiotFirmwareRolloutUpdatePayload::default());
    }
    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;
    Ok(AiotFirmwareRolloutUpdatePayload {
        target_policy_json: obj.get("targetPolicy").map(JsonValue::to_string),
        status: optional_json_string_field(obj, "status"),
    })
}

fn device_update_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotDeviceUpdatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(AiotDeviceUpdatePayload::default());
    }

    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;

    let display_name = optional_json_string_field(obj, "displayName");
    let status = optional_json_string_field(obj, "status");
    let metadata_json = obj.get("metadata").map(|value| value.to_string());

    Ok(AiotDeviceUpdatePayload {
        display_name,
        status,
        metadata_json,
    })
}

fn credential_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotCredentialCreatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.body.required",
            "Request body is required",
        ));
    }

    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;

    let credential_type = required_json_enum_field(
        obj,
        "credentialType",
        &["bearer_token", "hmac", "mtls_x509", "hardware_attestation"],
    )?;
    let expires_at = optional_json_string_field(obj, "expiresAt");

    Ok(AiotCredentialCreatePayload {
        credential_type,
        expires_at,
    })
}

fn twin_update_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotTwinUpdatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.body.required",
            "Request body is required",
        ));
    }

    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;

    let desired = parse_twin_update_section(obj, "desired")?;
    let reported = parse_twin_update_section(obj, "reported")?;
    if desired.is_empty() && reported.is_empty() {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            "Field desired or reported is required",
        ));
    }

    Ok(AiotTwinUpdatePayload { desired, reported })
}

fn parse_twin_update_section(
    obj: &JsonMap<String, JsonValue>,
    field: &str,
) -> Result<BTreeMap<String, String>, HttpResponse> {
    let Some(value) = obj.get(field) else {
        return Ok(BTreeMap::new());
    };
    let Some(section) = value.as_object() else {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            &format!("Field {field} must be a JSON object"),
        ));
    };
    Ok(section
        .iter()
        .map(|(key, value)| (key.clone(), value.to_string()))
        .collect())
}

fn required_json_string_field(
    obj: &JsonMap<String, JsonValue>,
    field: &str,
) -> Result<String, HttpResponse> {
    let value = obj.get(field).and_then(JsonValue::as_str).ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            &format!("Field {field} must be a non-empty string"),
        )
    })?;
    if value.trim().is_empty() {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            &format!("Field {field} must be a non-empty string"),
        ));
    }
    Ok(value.to_string())
}

fn required_json_enum_field(
    obj: &JsonMap<String, JsonValue>,
    field: &str,
    allowed_values: &[&str],
) -> Result<String, HttpResponse> {
    let value = required_json_string_field(obj, field)?;
    if allowed_values.contains(&value.as_str()) {
        return Ok(value);
    }

    Err(problem_response(
        HttpStatus::BadRequest,
        "api.request.invalid_field",
        &format!(
            "Field {field} must be one of: {}",
            allowed_values.join(", ")
        ),
    ))
}

fn required_json_int64_string_field(
    obj: &JsonMap<String, JsonValue>,
    field: &str,
) -> Result<String, HttpResponse> {
    let value = required_json_string_field(obj, field)?;
    if !is_valid_int64_string(&value) {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            &format!("Field {field} must be an int64 string"),
        ));
    }

    Ok(value)
}

fn is_valid_int64_string(value: &str) -> bool {
    if value.is_empty() || !value.as_bytes().iter().all(u8::is_ascii_digit) {
        return false;
    }

    value.parse::<i64>().is_ok()
}

fn optional_json_string_field(obj: &JsonMap<String, JsonValue>, field: &str) -> Option<String> {
    obj.get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_string)
}

fn optional_json_string_array_field(
    obj: &JsonMap<String, JsonValue>,
    field: &str,
) -> Result<Option<Vec<String>>, HttpResponse> {
    let Some(value) = obj.get(field) else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            &format!("Field {field} must be a string array"),
        ));
    };

    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let Some(item) = value.as_str() else {
            return Err(problem_response(
                HttpStatus::BadRequest,
                "api.request.invalid_field",
                &format!("Field {field} must be a string array"),
            ));
        };
        parsed.push(item.to_string());
    }
    Ok(Some(parsed))
}

fn optional_json_capability_definitions_field(
    obj: &JsonMap<String, JsonValue>,
    field: &str,
) -> Result<Option<Vec<CapabilityDefinition>>, HttpResponse> {
    let Some(value) = obj.get(field) else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            &format!("Field {field} must be an array"),
        ));
    };

    let mut definitions = Vec::with_capacity(values.len());
    for value in values {
        definitions.push(capability_definition_from_json(value)?);
    }
    Ok(Some(definitions))
}

fn capability_definition_from_json(
    value: &JsonValue,
) -> Result<CapabilityDefinition, HttpResponse> {
    let Some(obj) = value.as_object() else {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_field",
            "Capability definitions must be JSON objects",
        ));
    };

    let capability_name = required_json_string_field(obj, "capabilityName")
        .or_else(|_| required_json_string_field(obj, "name"))?;
    let capability_kind = required_json_string_field(obj, "capabilityKind")
        .or_else(|_| required_json_string_field(obj, "kind"))?;
    let mut definition = CapabilityDefinition::new(
        capability_name,
        capability_kind_from_name(&capability_kind).ok_or_else(|| {
            problem_response(
                HttpStatus::BadRequest,
                "api.request.invalid_field",
                "Field capabilityKind must be a known capability kind",
            )
        })?,
    );

    for command in optional_json_string_array_field(obj, "commands")?.unwrap_or_default() {
        definition = definition.with_command(command);
    }
    for event in optional_json_string_array_field(obj, "events")?.unwrap_or_default() {
        definition = definition.with_event(event);
    }
    if let Some(mappings) = obj.get("protocolMappings") {
        let Some(mappings) = mappings.as_array() else {
            return Err(problem_response(
                HttpStatus::BadRequest,
                "api.request.invalid_field",
                "Field protocolMappings must be an array",
            ));
        };
        for mapping in mappings {
            let Some(mapping) = mapping.as_object() else {
                return Err(problem_response(
                    HttpStatus::BadRequest,
                    "api.request.invalid_field",
                    "Protocol mappings must be JSON objects",
                ));
            };
            let protocol_id = required_json_string_field(mapping, "protocolId")?;
            let mapped_name = required_json_string_field(mapping, "mappedName")?;
            definition = definition.with_protocol_mapping(protocol_id, mapped_name);
        }
    }

    Ok(definition)
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

#[derive(Debug, Clone)]
struct AiotCommandCreatePayload {
    capability_name: String,
    command_name: String,
    payload_json: String,
    request_media_resource_id: Option<String>,
    request_object_blob_id: Option<String>,
    request_media_json: Option<String>,
    session_id: Option<String>,
    trace_id: Option<String>,
    timeout_at: Option<String>,
    idempotency_key: Option<String>,
}

fn command_create_payload_from_request(
    request: &HttpRequest,
) -> Result<AiotCommandCreatePayload, HttpResponse> {
    if request.body.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Err(problem_response(
            HttpStatus::BadRequest,
            "api.request.body.required",
            "Request body is required",
        ));
    }

    let body: JsonValue = serde_json::from_slice(&request.body).map_err(|_| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json",
            "Request body must be valid JSON",
        )
    })?;
    let obj = body.as_object().ok_or_else(|| {
        problem_response(
            HttpStatus::BadRequest,
            "api.request.invalid_json_object",
            "Request body must be a JSON object",
        )
    })?;

    let capability_name = required_json_string_field(obj, "capabilityName")?;
    let command_name = required_json_string_field(obj, "commandName")?;
    let payload_json = obj
        .get("payload")
        .map(JsonValue::to_string)
        .ok_or_else(|| {
            problem_response(
                HttpStatus::BadRequest,
                "api.request.invalid_field",
                "Field payload is required",
            )
        })?;

    let mut request_media_resource_id = optional_json_string_field(obj, "requestMediaResourceId");
    let mut request_object_blob_id = optional_json_string_field(obj, "requestObjectBlobId");
    let mut request_media_json = obj.get("requestMedia").map(JsonValue::to_string);
    if let Some(value) = obj.get("requestMedia") {
        request_media_json = Some(value.to_string());
        if let Some(media_id) = value
            .as_object()
            .and_then(|media| json_object_string_field(media, "id"))
        {
            request_media_resource_id = Some(media_id.to_string());
        }
        if let Some(blob_id) = value
            .as_object()
            .and_then(|media| json_object_string_field(media, "objectBlobId"))
        {
            request_object_blob_id = Some(blob_id.to_string());
        }
    }

    let idempotency_key = request
        .header("idempotency-key")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| optional_json_string_field(obj, "idempotencyKey"));

    Ok(AiotCommandCreatePayload {
        capability_name,
        command_name,
        payload_json,
        request_media_resource_id,
        request_object_blob_id,
        request_media_json,
        session_id: optional_json_string_field(obj, "sessionId"),
        trace_id: optional_json_string_field(obj, "traceId"),
        timeout_at: optional_json_string_field(obj, "timeoutAt"),
        idempotency_key,
    })
}

fn json_object_string_field<'a>(
    obj: &'a JsonMap<String, JsonValue>,
    field: &str,
) -> Option<&'a str> {
    obj.get(field).and_then(JsonValue::as_str)
}

fn standard_command_collection_response(
    request: &HttpRequest,
    commands: &[AiotCommandRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = commands
        .iter()
        .map(command_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn standard_command_response(
    request: &HttpRequest,
    status: HttpStatus,
    command: &AiotCommandRecord,
) -> HttpResponse {
    standard_resource_response(request, status, command_resource_json(command))
}

fn command_resource_json(command: &AiotCommandRecord) -> String {
    let result_json = command
        .result
        .as_ref()
        .map(command_result_json)
        .unwrap_or_else(|| "null".to_string());

    format!(
        r#"{{"commandId":"{}","deviceId":"{}","sessionId":{},"capabilityName":"{}","commandName":"{}","requestPayload":{},"requestMediaResourceId":{},"requestObjectBlobId":{},"requestMedia":{},"status":"{}","traceId":{},"timeoutAt":{},"ackAt":{},"resultAt":{},"createdAt":"{}","result":{}}}"#,
        json_escape(&command.command_id),
        json_escape(&command.device_id),
        json_string_or_null(command.session_id.as_deref()),
        json_escape(&command.capability_name),
        json_escape(&command.command_name),
        json_value_or_string(&command.request_payload_json),
        json_string_or_null(command.request_media_resource_id.as_deref()),
        json_string_or_null(command.request_object_blob_id.as_deref()),
        json_raw_or_null(command.request_media_json.as_deref()),
        json_escape(&command.status),
        json_string_or_null(command.trace_id.as_deref()),
        json_string_or_null(command.timeout_at.as_deref()),
        json_string_or_null(command.ack_at.as_deref()),
        json_string_or_null(command.result_at.as_deref()),
        json_escape(&command.created_at),
        result_json,
    )
}

fn command_result_json(result: &sdkwork_aiot_storage::AiotCommandResultRecord) -> String {
    format!(
        r#"{{"resultCode":{},"resultPayload":{},"resultMediaResourceId":{},"resultObjectBlobId":{},"resultMedia":{},"occurredAt":{}}}"#,
        json_string_or_null(result.result_code.as_deref()),
        json_raw_or_null(result.result_payload_json.as_deref()),
        json_string_or_null(result.result_media_resource_id.as_deref()),
        json_string_or_null(result.result_object_blob_id.as_deref()),
        json_raw_or_null(result.result_media_json.as_deref()),
        json_string_or_null(result.occurred_at.as_deref()),
    )
}

fn standard_event_collection_response(
    request: &HttpRequest,
    events: &[AiotDeviceEventRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = events
        .iter()
        .map(event_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn event_resource_json(event: &AiotDeviceEventRecord) -> String {
    format!(
        r#"{{"eventId":"{}","eventType":"{}","eventVersion":"{}","deviceId":"{}","protocolId":"{}","adapterId":"{}","messageClass":"{}","semanticType":"{}","transport":"{}","direction":"{}","messageId":{},"correlationId":{},"traceId":{},"payloadHash":{},"mediaResourceId":{},"objectBlobId":{},"media":{},"payload":{},"occurredAt":"{}"}}"#,
        json_escape(&event.event_id),
        json_escape(&event.event_type),
        json_escape(&event.event_version),
        json_escape(&event.device_id),
        json_escape(&event.protocol_id),
        json_escape(&event.adapter_id),
        json_escape(&event.message_class),
        json_escape(&event.semantic_type),
        json_escape(&event.transport),
        json_escape(&event.direction),
        json_string_or_null(event.message_id.as_deref()),
        json_string_or_null(event.correlation_id.as_deref()),
        json_string_or_null(event.trace_id.as_deref()),
        json_string_or_null(event.payload_hash.as_deref()),
        json_string_or_null(event.media_resource_id.as_deref()),
        json_string_or_null(event.object_blob_id.as_deref()),
        json_raw_or_null(event.media_json.as_deref()),
        json_value_or_string(&event.payload_json),
        json_escape(&event.occurred_at),
    )
}

fn standard_twin_response(
    request: &HttpRequest,
    snapshot: &AiotDeviceTwinSnapshot,
) -> HttpResponse {
    standard_resource_response(
        request,
        HttpStatus::Ok,
        format!(
            r#"{{"deviceId":"{}","desired":{},"reported":{},"desiredVersion":"{}","reportedVersion":"{}","updatedAt":"{}"}}"#,
            json_escape(&snapshot.device_id),
            json_map_with_json_values(&snapshot.desired),
            json_map_with_json_values(&snapshot.reported),
            snapshot.desired_version,
            snapshot.reported_version,
            json_escape(&snapshot.updated_at),
        ),
    )
}

fn standard_device_session_collection_response(
    request: &HttpRequest,
    sessions: &[AiotDeviceSessionRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = sessions
        .iter()
        .map(device_session_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn device_session_resource_json(session: &AiotDeviceSessionRecord) -> String {
    format!(
        r#"{{"sessionId":"{}","deviceId":"{}","status":"{}","connectedAt":{},"disconnectedAt":{},"transport":"{}"}}"#,
        json_escape(&session.session_id),
        json_escape(&session.device_id),
        json_escape(&session.status),
        json_string_or_null(session.connected_at.as_deref()),
        json_string_or_null(session.disconnected_at.as_deref()),
        json_escape(&session.transport),
    )
}

fn standard_device_capability_collection_response(
    request: &HttpRequest,
    capabilities: &[AiotDeviceCapabilityRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = capabilities
        .iter()
        .map(device_capability_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn standard_device_credential_response(
    request: &HttpRequest,
    status: HttpStatus,
    credential: &AiotDeviceCredentialRecord,
) -> HttpResponse {
    standard_resource_response(request, status, device_credential_resource_json(credential))
}

fn standard_device_credential_collection_response(
    request: &HttpRequest,
    credentials: &[AiotDeviceCredentialRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = credentials
        .iter()
        .map(device_credential_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn device_credential_resource_json(credential: &AiotDeviceCredentialRecord) -> String {
    let issued_secret = credential
        .issued_secret
        .as_deref()
        .map(|secret| format!(r#","issuedSecret":"{}""#, json_escape(secret)))
        .unwrap_or_default();
    format!(
        r#"{{"credentialId":"{}","deviceId":"{}","credentialType":"{}","status":"{}","expiresAt":{},"createdAt":"{}","revokedAt":{}{}}}"#,
        json_escape(&credential.credential_id),
        json_escape(&credential.device_id),
        json_escape(&credential.credential_type),
        json_escape(&credential.status),
        json_string_or_null(credential.expires_at.as_deref()),
        json_escape(&credential.created_at),
        json_string_or_null(credential.revoked_at.as_deref()),
        issued_secret
    )
}

fn device_capability_resource_json(capability: &AiotDeviceCapabilityRecord) -> String {
    format!(
        r#"{{"capabilityName":"{}","capabilityKind":"{}","status":"{}"}}"#,
        json_escape(&capability.capability_name),
        json_escape(&capability.capability_kind),
        json_escape(&capability.status),
    )
}

fn standard_firmware_artifact_response(
    request: &HttpRequest,
    status: HttpStatus,
    artifact: &AiotFirmwareArtifactRecord,
) -> HttpResponse {
    standard_resource_response(request, status, firmware_artifact_resource_json(artifact))
}

fn standard_firmware_artifact_collection_response(
    request: &HttpRequest,
    artifacts: &[AiotFirmwareArtifactRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = artifacts
        .iter()
        .map(firmware_artifact_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn firmware_artifact_resource_json(artifact: &AiotFirmwareArtifactRecord) -> String {
    format!(
        r#"{{"artifactId":"{}","artifactKey":"{}","version":"{}","mediaResourceId":"{}","resource":{},"objectBlobId":{},"sha256":"{}","signature":{},"targetChipFamily":{},"targetRuntimeProfile":{},"status":"{}"}}"#,
        json_escape(&artifact.artifact_id),
        json_escape(&artifact.artifact_key),
        json_escape(&artifact.version),
        json_escape(&artifact.media_resource_id),
        json_value_or_string(&artifact.resource_json),
        media_resource_object_blob_id(&artifact.resource_json),
        json_escape(&artifact.sha256),
        json_string_or_null(artifact.signature.as_deref()),
        json_string_or_null(artifact.target_chip_family.as_deref()),
        json_string_or_null(artifact.target_runtime_profile.as_deref()),
        json_escape(&artifact.status)
    )
}

fn standard_firmware_rollout_response(
    request: &HttpRequest,
    status: HttpStatus,
    rollout: &AiotFirmwareRolloutRecord,
) -> HttpResponse {
    standard_resource_response(request, status, firmware_rollout_resource_json(rollout))
}

fn standard_firmware_rollout_collection_response(
    request: &HttpRequest,
    rollouts: &[AiotFirmwareRolloutRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = rollouts
        .iter()
        .map(firmware_rollout_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn firmware_rollout_resource_json(rollout: &AiotFirmwareRolloutRecord) -> String {
    format!(
        r#"{{"rolloutId":"{}","artifactId":"{}","targetPolicy":{},"status":"{}"}}"#,
        json_escape(&rollout.rollout_id),
        json_escape(&rollout.artifact_id),
        json_value_or_string(&rollout.target_policy_json),
        json_escape(&rollout.status)
    )
}

fn standard_device_response(
    request: &HttpRequest,
    status: HttpStatus,
    device: &AiotDeviceRecord,
) -> HttpResponse {
    standard_resource_response(request, status, device_resource_json(device))
}

fn standard_device_collection_response(
    request: &HttpRequest,
    devices: &[AiotDeviceRecord],
    page_query: PageQuery,
    total: i64,
) -> HttpResponse {
    let items = devices
        .iter()
        .map(device_resource_json)
        .collect::<Vec<_>>()
        .join(",");
    json_collection_response(request, &items, page_query, total)
}

fn device_resource_json(device: &AiotDeviceRecord) -> String {
    format!(
        r#"{{"id":"{}","tenantId":"{}","organizationId":"{}","deviceId":"{}","displayName":"{}","productId":"{}","clientId":{},"chipFamily":{},"status":"{}","metadata":{},"lastSeenAt":"{}"}}"#,
        json_escape(&device.id),
        device.tenant_id,
        device.organization_id,
        json_escape(&device.device_id),
        json_escape(&device.display_name),
        json_escape(&device.product_id),
        json_string_or_null(device.client_id.as_deref()),
        json_string_or_null(device.chip_family.as_deref()),
        json_escape(&device.status),
        device.metadata_json.as_deref().unwrap_or("null"),
        json_escape(&device.last_seen_at),
    )
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

fn protocol_scope_name(scope: ProtocolPluginScope) -> &'static str {
    match scope {
        ProtocolPluginScope::StandardAdapter => "StandardAdapter",
        ProtocolPluginScope::CompatibilityPlugin => "CompatibilityPlugin",
        ProtocolPluginScope::BridgeAdapter => "BridgeAdapter",
    }
}

fn capability_bridge_name(bridge: &CapabilityBridge) -> &'static str {
    match bridge {
        CapabilityBridge::StandardCapability => "standard_capability",
        CapabilityBridge::McpJsonRpc => "mcp_jsonrpc",
        CapabilityBridge::Lwm2mObject => "lwm2m_object",
        CapabilityBridge::MatterCluster => "matter_cluster",
        CapabilityBridge::ZigbeeCluster => "zigbee_cluster",
        CapabilityBridge::LorawanPayloadCodec => "lorawan_payload_codec",
        CapabilityBridge::RegisterMap => "register_map",
        CapabilityBridge::OpcUaNode => "opcua_node",
        CapabilityBridge::MqttTopic => "mqtt_topic",
        CapabilityBridge::FirmwareOta => "firmware_ota",
    }
}

fn json_string_or_null(value: Option<&str>) -> String {
    value
        .map(|value| format!(r#""{}""#, json_escape(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn json_raw_or_null(value: Option<&str>) -> String {
    value
        .map(json_value_or_string)
        .unwrap_or_else(|| "null".to_string())
}

fn json_value_or_string(value: &str) -> String {
    if serde_json::from_str::<JsonValue>(value).is_ok() {
        value.to_string()
    } else {
        format!(r#""{}""#, json_escape(value))
    }
}

fn media_resource_object_blob_id(resource_json: &str) -> String {
    serde_json::from_str::<JsonValue>(resource_json)
        .ok()
        .and_then(|value| {
            value
                .as_object()
                .and_then(|obj| json_object_string_field(obj, "objectBlobId"))
                .map(str::to_string)
        })
        .map(|value| format!(r#""{}""#, json_escape(&value)))
        .unwrap_or_else(|| "null".to_string())
}

fn json_map_with_json_values(values: &BTreeMap<String, String>) -> String {
    let items = values
        .iter()
        .map(|(key, value)| format!(r#""{}":{}"#, json_escape(key), json_value_or_string(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{items}}}")
}

fn device_not_found_response(device_id: &str) -> HttpResponse {
    domain_not_found_response("Device", device_id, None)
}

fn product_not_found_response(product_id: &str) -> HttpResponse {
    domain_not_found_response("Product", product_id, None)
}

fn hardware_profile_not_found_response(hardware_profile_id: &str) -> HttpResponse {
    domain_not_found_response("Hardware profile", hardware_profile_id, None)
}

fn protocol_profile_not_found_response(protocol_profile_id: &str) -> HttpResponse {
    domain_not_found_response("Protocol profile", protocol_profile_id, None)
}

fn capability_model_not_found_response(capability_model_id: &str) -> HttpResponse {
    domain_not_found_response("Capability model", capability_model_id, None)
}

fn credential_not_found_response(credential_id: &str) -> HttpResponse {
    domain_not_found_response("Credential", credential_id, None)
}

fn device_session_not_found_response(session_id: &str) -> HttpResponse {
    domain_not_found_response("Device session", session_id, None)
}

fn command_not_found_response(command_id: &str) -> HttpResponse {
    domain_not_found_response("Command", command_id, None)
}

fn firmware_artifact_not_found_response(artifact_id: &str) -> HttpResponse {
    domain_not_found_response("Firmware artifact", artifact_id, None)
}

fn firmware_rollout_not_found_response(rollout_id: &str) -> HttpResponse {
    domain_not_found_response("Firmware rollout", rollout_id, None)
}

fn problem_response(status: HttpStatus, code: &str, title: &str) -> HttpResponse {
    let _ = status;
    problem_detail_from_wire_code(None, code, title)
}

fn permission_denied_response(required_permission: &str) -> HttpResponse {
    problem_detail_from_wire_code(
        None,
        "api.permission.denied",
        format!("Permission denied: {required_permission}"),
    )
}

pub(crate) fn apply_media_object_blob_id(
    resource_json: &str,
    object_blob_id: &str,
) -> Result<String, serde_json::Error> {
    let mut value: JsonValue = serde_json::from_str(resource_json)?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "objectBlobId".to_string(),
            JsonValue::String(object_blob_id.to_string()),
        );
    }
    serde_json::to_string(&value)
}

fn debug_array<'a, T, I>(values: I) -> String
where
    T: std::fmt::Debug + 'a,
    I: IntoIterator<Item = &'a T>,
{
    values
        .into_iter()
        .map(|value| format!(r#""{value:?}""#))
        .collect::<Vec<_>>()
        .join(",")
}

fn string_array<'a, I>(values: I) -> String
where
    I: IntoIterator<Item = &'a String>,
{
    values
        .into_iter()
        .map(|value| format!(r#""{}""#, json_escape(value)))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(feature = "intelligence-kernel")]
fn assistant_chat_user_text(payload_json: &str) -> String {
    serde_json::from_str::<JsonValue>(payload_json)
        .ok()
        .and_then(|value| {
            value
                .get("text")
                .or_else(|| value.get("content"))
                .and_then(JsonValue::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[cfg(feature = "intelligence-kernel")]
fn current_rfc3339_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn route_kind_name(kind: sdkwork_aiot_service_host::AiotProtocolRouteKind) -> &'static str {
    match kind {
        sdkwork_aiot_service_host::AiotProtocolRouteKind::DeviceSession => "deviceSession",
        sdkwork_aiot_service_host::AiotProtocolRouteKind::OtaMetadata => "otaMetadata",
        sdkwork_aiot_service_host::AiotProtocolRouteKind::Provisioning => "provisioning",
        sdkwork_aiot_service_host::AiotProtocolRouteKind::BridgeIngress => "bridgeIngress",
        sdkwork_aiot_service_host::AiotProtocolRouteKind::Callback => "callback",
    }
}

fn parse_http_request(bytes: &[u8]) -> Result<HttpRequest, AiotApiError> {
    let header_len = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|idx| idx + 4)
        .ok_or_else(|| AiotApiError::new("api.http.incomplete_headers"))?;
    let raw = std::str::from_utf8(&bytes[..header_len])
        .map_err(|_| AiotApiError::new("api.http.invalid_utf8"))?;
    let mut lines = raw.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| AiotApiError::new("api.http.empty"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| AiotApiError::new("api.http.missing_method"))?;
    let path = parts
        .next()
        .ok_or_else(|| AiotApiError::new("api.http.missing_path"))?;
    let (path_only, query) = path.split_once('?').unwrap_or((path, ""));
    let mut request = HttpRequest::new(method, path_only);
    request.raw_path = path.to_string();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        request = request.with_query_param(name, value);
    }

    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            request = request.with_header(name.trim(), value.trim());
        }
    }

    request.body.extend_from_slice(&bytes[header_len..]);

    Ok(request)
}

fn format_http_response(response: &HttpResponse) -> String {
    let response = apply_security_headers(response.clone());
    let mut out = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status.code(),
        response.status.reason()
    );
    for (name, value) in response.headers() {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
    out.push_str("content-length: ");
    out.push_str(response.body.len().to_string().as_str());
    out.push_str("\r\n\r\n");
    out.push_str(&response.body);
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotApiError {
    pub code: String,
}

impl AiotApiError {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}
