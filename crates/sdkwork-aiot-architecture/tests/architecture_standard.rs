use std::fs;
use std::path::{Path, PathBuf};

use sdkwork_iot_platform_service::{standard_api_route_contracts, AiotApiSurface};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn quoted_json_values_after_key(document: &str, key: &str) -> Vec<String> {
    let marker = format!(r#""{key}": ""#);
    document
        .match_indices(&marker)
        .filter_map(|(start, _)| {
            let value_start = start + marker.len();
            let rest = &document[value_start..];
            let value_end = rest.find('"')?;
            Some(rest[..value_end].to_string())
        })
        .collect()
}

fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}

fn topology_retired_env_keys(document: &str) -> Vec<String> {
    let parsed: serde_json::Value = serde_json::from_str(strip_utf8_bom(document))
        .expect("topology.spec.json must be valid JSON");
    parsed["retired"]["envKeys"]
        .as_array()
        .expect("topology.spec.json retired.envKeys must be an array")
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .expect("topology.spec.json retired.envKeys entries must be strings")
                .to_string()
        })
        .collect()
}

fn openapi_permission_for_operation(document: &str, operation_id: &str) -> Option<String> {
    let operation_marker = format!(r#""operationId": "{operation_id}""#);
    let operation_start = document.find(&operation_marker)?;
    let rest = &document[operation_start..];
    let permission_marker = r#""x-sdkwork-required-permission": ""#;
    let permission_start = rest.find(permission_marker)? + permission_marker.len();
    let permission_rest = &rest[permission_start..];
    let permission_end = permission_rest.find('"')?;

    Some(permission_rest[..permission_end].to_string())
}

#[test]
fn github_packaging_workflow_is_declared() {
    let root = workspace_root();
    let workflow = root.join("sdkwork.workflow.json");
    let package_workflow = root.join(".github/workflows/package.yml");

    assert!(workflow.exists(), "sdkwork.workflow.json is required");
    let workflow_text = fs::read_to_string(&workflow).expect("sdkwork.workflow.json");
    assert!(workflow_text.contains(r#""id": "sdkwork-aiot""#));
    assert!(workflow_text.contains("sdkwork-web-framework"));
    assert!(workflow_text.contains("sdkwork-database"));

    assert!(
        package_workflow.exists(),
        ".github/workflows/package.yml is required"
    );
    let package_text = fs::read_to_string(&package_workflow).expect("package workflow");
    assert!(package_text.contains("sdkwork.workflow.json"));
    assert!(package_text.contains("sdkwork-github-workflow"));
}

#[test]
fn route_manifests_declare_web_request_context_metadata() {
    let root = workspace_root();
    let cases = [
        (
            "sdks/_route-manifests/app-api/sdkwork-aiot-app-api.route-manifest.json",
            "app-api",
        ),
        (
            "sdks/_route-manifests/backend-api/sdkwork-aiot-admin-api.route-manifest.json",
            "backend-api",
        ),
    ];

    for (relative_path, expected_surface) in cases {
        let manifest = fs::read_to_string(root.join(relative_path)).expect(relative_path);
        assert!(
            manifest.contains(r#""requestContext": "WebRequestContext""#),
            "{relative_path} must declare requestContext"
        );
        assert!(
            manifest.contains(&format!(r#""apiSurface": "{expected_surface}""#)),
            "{relative_path} must declare apiSurface={expected_surface}"
        );
    }
}

#[test]
fn openapi_authorities_declare_web_request_context_extensions() {
    let root = workspace_root();
    let cases = [
        (
            "apis/app-api/iot/sdkwork-aiot-app-api.openapi.json",
            "app-api",
        ),
        (
            "apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json",
            "backend-api",
        ),
    ];

    for (relative_path, expected_surface) in cases {
        let openapi = fs::read_to_string(root.join(relative_path)).expect(relative_path);
        assert!(
            openapi.contains(r#""x-sdkwork-request-context": "WebRequestContext""#),
            "{relative_path} must declare x-sdkwork-request-context"
        );
        assert!(
            openapi.contains(&format!(r#""x-sdkwork-api-surface": "{expected_surface}""#)),
            "{relative_path} must declare x-sdkwork-api-surface"
        );
        assert!(
            openapi.contains(r#""x-sdkwork-owner": "sdkwork-aiot""#),
            "{relative_path} must declare x-sdkwork-owner"
        );
        let expected_authority = if expected_surface == "app-api" {
            "sdkwork-aiot-app-api"
        } else {
            "sdkwork-aiot-backend-api"
        };
        assert!(
            openapi.contains(&format!(
                r#""x-sdkwork-api-authority": "{expected_authority}""#
            )),
            "{relative_path} must declare x-sdkwork-api-authority"
        );
        assert!(
            openapi.contains(r#""PageInfo""#),
            "{relative_path} must declare PageInfo schema for list pagination"
        );
    }
}

#[test]
fn apis_authority_inputs_exist_and_sdk_assemblies_reference_them() {
    let root = workspace_root();
    let authorities = [
        "apis/app-api/iot/sdkwork-aiot-app-api.openapi.json",
        "apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json",
    ];

    for relative_path in authorities {
        assert!(
            root.join(relative_path).exists(),
            "{relative_path} is required authored API authority input"
        );
    }

    for (assembly_path, authority_path) in [
        (
            "sdks/sdkwork-aiot-app-sdk/.sdkwork-assembly.json",
            "../../apis/app-api/iot/sdkwork-aiot-app-api.openapi.json",
        ),
        (
            "sdks/sdkwork-aiot-backend-sdk/.sdkwork-assembly.json",
            "../../apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json",
        ),
    ] {
        let assembly = fs::read_to_string(root.join(assembly_path)).expect(assembly_path);
        assert!(
            assembly.contains(authority_path),
            "{assembly_path} must reference {authority_path}"
        );
    }
}

#[test]
fn service_shells_bootstrap_shared_device_database() {
    let root = workspace_root();

    for (service, bootstrap_fn) in [
        (
            "services/sdkwork-aiot-app-api/src/main.rs",
            "open_app_service_stores",
        ),
        (
            "services/sdkwork-aiot-admin-api/src/main.rs",
            "open_admin_service_stores",
        ),
    ] {
        let source = fs::read_to_string(root.join(service)).expect(service);
        assert!(
            source.contains(bootstrap_fn),
            "{service} must bootstrap persistence through {bootstrap_fn}"
        );
        assert!(
            !source.contains("open_device_repository("),
            "{service} must not open separate device repository pools"
        );
    }
}

#[test]
fn github_packaging_workflow_declares_sdkwork_utils() {
    let workflow_text =
        fs::read_to_string(workspace_root().join("sdkwork.workflow.json")).expect("workflow");
    assert!(workflow_text.contains("sdkwork-utils"));
}

#[test]
fn root_package_json_exposes_standard_pnpm_scripts() {
    let package_json =
        fs::read_to_string(workspace_root().join("package.json")).expect("package.json");
    for script in [
        "\"dev\"",
        "\"build\"",
        "\"test\"",
        "\"check\"",
        "\"verify\"",
        "\"clean\"",
        "\"topology:validate\"",
        "\"api:check\"",
        "\"api:materialize\"",
        "\"api:materialize:check\"",
        "\"sdk:generate\"",
        "\"sdk:generate:check\"",
        "\"sdk:check\"",
        "\"gateway:run\"",
        "\"gateway:validate\"",
        "\"gateway:plan\"",
        "\"release:build\"",
        "\"release:validate\"",
        "\"release:plan\"",
        "\"deploy:plan\"",
        "\"deploy:validate\"",
        "\"sbom:generate\"",
        "\"sbom:check\"",
        "\"topology:plan\"",
        "\"test:workspace-standard\"",
    ] {
        assert!(
            package_json.contains(script),
            "package.json must expose standard script {script}"
        );
    }
}

#[test]
fn shared_app_core_declares_sdkwork_utils_typescript() {
    let root = workspace_root();
    let shared_workspace =
        fs::read_to_string(root.join("apps/sdkwork-aiot-shared/pnpm-workspace.yaml"))
            .expect("shared pnpm workspace");
    assert!(
        shared_workspace.contains("sdkwork-utils-typescript"),
        "shared pnpm workspace must include sdkwork-utils-typescript"
    );

    let app_core_package = fs::read_to_string(
        root.join("apps/sdkwork-aiot-shared/packages/sdkwork-aiot-app-core/package.json"),
    )
    .expect("app-core package.json");
    assert!(
        app_core_package.contains("@sdkwork/utils"),
        "app-core must depend on @sdkwork/utils"
    );
    assert!(
        root.join("tools/aiot_sdk_generate.mjs").exists(),
        "tools/aiot_sdk_generate.mjs is required for sdk:generate and sdk:check"
    );
}

#[test]
fn shared_app_core_exports_runtime_env_helpers() {
    let root = workspace_root();
    let app_core_index = fs::read_to_string(
        root.join("apps/sdkwork-aiot-shared/packages/sdkwork-aiot-app-core/src/index.ts"),
    )
    .expect("app-core index");
    let runtime_env =
        fs::read_to_string(root.join(
            "apps/sdkwork-aiot-shared/packages/sdkwork-aiot-app-core/src/utils/runtimeEnv.ts",
        ))
        .expect("runtime env helpers");

    assert!(runtime_env.contains("@sdkwork/utils"));
    assert!(runtime_env.contains("readImportMetaEnv"));
    assert!(runtime_env.contains("readProcessEnv"));
    assert!(app_core_index.contains("readImportMetaEnv"));
    assert!(app_core_index.contains("readOptionalBearerToken"));
}

#[test]
fn standards_alignment_roadmap_is_documented() {
    let root = workspace_root();
    let adr = root.join("docs/adr/004-standards-alignment-roadmap.md");
    assert!(adr.exists(), "standards alignment ADR is required");
    let adr_text = fs::read_to_string(&adr).expect("standards alignment ADR");
    assert!(adr_text.contains("sdkwork-web-framework"));
    assert!(adr_text.contains("sdkwork-database"));
    assert!(adr_text.contains("sdkwork-utils"));
    assert!(adr_text.contains("sdkwork-discovery"));
}

#[test]
fn service_shells_mount_sdkwork_web_framework_routers() {
    let root = workspace_root();

    for (service, router_crate) in [
        (
            "services/sdkwork-aiot-app-api/src/main.rs",
            "sdkwork_routes_iot_app_api",
        ),
        (
            "services/sdkwork-aiot-admin-api/src/main.rs",
            "sdkwork_routes_iot_backend_api",
        ),
    ] {
        let source = fs::read_to_string(root.join(service)).expect(service);
        assert!(
            source.contains(router_crate),
            "{service} must mount HTTP APIs through {router_crate}"
        );
        assert!(
            source.contains("tokio::main") || source.contains("#[tokio::main]"),
            "{service} must use async Tokio runtime for sdkwork-web-framework"
        );
        assert!(
            !source.contains("sdkwork_aiot_transport::serve_http_concurrent"),
            "{service} must not use legacy transport server"
        );
    }
}

#[test]
fn workspace_declares_sdkwork_web_framework_dependencies() {
    let root = workspace_root();
    let cargo = fs::read_to_string(root.join("Cargo.toml")).expect("workspace Cargo.toml");

    for dependency in [
        "sdkwork-web-axum",
        "sdkwork-web-core",
        "sdkwork-iam-web-adapter",
        "sdkwork-database-config",
        "sdkwork-database-sqlx",
        "sdkwork-utils-rust",
    ] {
        assert!(
            cargo.contains(dependency),
            "workspace Cargo.toml must declare {dependency}"
        );
    }

    for crate_dir in [
        "crates/sdkwork-routes-iot-app-api",
        "crates/sdkwork-routes-iot-backend-api",
        "crates/sdkwork-aiot-app-context",
    ] {
        assert!(
            root.join(crate_dir).join("Cargo.toml").exists(),
            "{crate_dir} is required for WEB_FRAMEWORK_SPEC alignment"
        );
    }
}

#[test]
fn device_storage_uses_sdkwork_database_bootstrap() {
    let root = workspace_root();
    let storage = fs::read_to_string(root.join("crates/sdkwork-aiot-storage-sqlx/src/lib.rs"))
        .expect("storage sqlx source");
    let bootstrap =
        fs::read_to_string(root.join("crates/sdkwork-aiot-storage-sqlx/src/database_bootstrap.rs"))
            .expect("database bootstrap source");

    assert!(storage.contains("mod database_bootstrap"));
    assert!(storage.contains("open_device_repository"));
    assert!(bootstrap.contains("DatabaseConfig"));
    assert!(bootstrap.contains("sdkwork_database_sqlx"));
    assert!(bootstrap.contains("create_pool_from_config"));
    assert!(bootstrap.contains("aiot_device_blocking_pool"));
    assert!(bootstrap.contains(r#"table_prefix: "iot_""#));
}

#[test]
fn workspace_does_not_depend_on_rusqlite() {
    let root = workspace_root();
    let mut cargo_files = vec![root.join("Cargo.toml")];
    for entry in fs::read_dir(root.join("crates")).expect("crates directory") {
        let entry = entry.expect("crate entry");
        if entry.path().is_dir() {
            cargo_files.push(entry.path().join("Cargo.toml"));
        }
    }
    for entry in fs::read_dir(root.join("services")).expect("services directory") {
        let entry = entry.expect("service entry");
        if entry.path().is_dir() {
            cargo_files.push(entry.path().join("Cargo.toml"));
        }
    }

    for cargo_path in cargo_files {
        if !cargo_path.exists() {
            continue;
        }
        let cargo = fs::read_to_string(&cargo_path).expect("Cargo.toml");
        assert!(
            !cargo.contains("rusqlite"),
            "{} must not depend on direct rusqlite; use sdkwork-database-sqlx",
            cargo_path.display()
        );
    }
}

#[test]
fn workspace_does_not_use_forbidden_crate_names() {
    let root = workspace_root();
    let forbidden = [
        "sdkwork-aiot-core",
        "sdkwork-aiot-runtime",
        "sdkwork-aiot-backend",
        "sdkwork-aiot-common",
        "sdkwork-aiot-manager",
        "sdkwork-aiot-product",
        "sdkwork-aiot-server-runtime",
    ];

    for entry in fs::read_dir(root.join("crates")).expect("crates directory") {
        let entry = entry.expect("crate entry");
        if !entry.path().is_dir() {
            continue;
        }
        let crate_name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !forbidden.contains(&crate_name.as_str()),
            "forbidden crate directory {crate_name}; use responsibility-specific names per NAMING_SPEC.md"
        );
    }

    let workspace_cargo =
        fs::read_to_string(root.join("Cargo.toml")).expect("workspace Cargo.toml");
    for name in forbidden {
        assert!(
            !workspace_cargo.contains(name),
            "workspace Cargo.toml must not reference forbidden crate {name}"
        );
    }
}

#[test]
fn app_manifest_declares_server_rust_workspace() {
    let manifest =
        fs::read_to_string(workspace_root().join("sdkwork.app.config.json")).expect("app manifest");

    assert!(manifest.contains(r#""appType": "APP_SERVICE""#));
    assert!(manifest.contains(r#""family": "server""#));
    assert!(manifest.contains(r#""framework": "rust-axum""#));
    assert!(manifest.contains(r#""workspaceRoot": ".""#));
    assert!(
        !manifest.contains("apps/sdkwork-aiot-pc"),
        "server workspace manifest must not point at a React PC client root"
    );
}

#[test]
fn workspace_does_not_create_parallel_aiot_iam_component() {
    let root = workspace_root();
    let cargo = fs::read_to_string(root.join("Cargo.toml")).expect("workspace Cargo.toml");

    assert!(!cargo.contains("sdkwork-aiot-iam"));
    assert!(!root.join("crates").join("sdkwork-aiot-iam").exists());
}

#[test]
fn service_shells_reuse_runtime_builder_instead_of_owning_domain_logic() {
    let root = workspace_root();

    for service in [
        "services/sdkwork-aiot-cloud-gateway/src/main.rs",
        "services/sdkwork-aiot-admin-api/src/main.rs",
        "services/sdkwork-aiot-app-api/src/main.rs",
    ] {
        let source = fs::read_to_string(root.join(service)).expect(service);

        assert!(
            source.contains("standard_aiot_runtime")
                || source.contains("standard_standalone")
                || source.contains("standard_gateway_server")
                || source.contains("standard_admin_api_server")
                || source.contains("standard_app_api_server"),
            "{service} must assemble a shared runtime-backed component"
        );
        assert!(
            !source.contains("struct Device") && !source.contains("struct Product"),
            "{service} must not define domain entities"
        );
        assert!(
            !source.contains("CREATE TABLE"),
            "{service} must not own database DDL"
        );

        if service.contains("admin-api") || service.contains("app-api") {
            assert!(
                source.contains("sdkwork_iot_platform_service"),
                "{service} must route through the shared HTTP API component"
            );
            assert!(
                source.contains("sdkwork_routes_iot"),
                "{service} must mount sdkwork-web-framework routers"
            );
            assert!(
                !source.contains("/backend/v3/api/iot/protocol_adapters")
                    && !source.contains("/app/v3/api/iot/devices"),
                "{service} must not inline app/backend API route behavior"
            );
        }
    }
}

#[test]
fn local_component_specs_exist_for_sdkwork_discovery() {
    let root = workspace_root();
    let readme = root.join("specs").join("README.md");
    let manifest = root.join("specs").join("component.spec.json");
    let manifest_text = fs::read_to_string(&manifest).expect("component spec manifest");

    assert!(readme.exists(), "specs/README.md is required");
    assert!(manifest.exists(), "specs/component.spec.json is required");
    assert!(manifest_text.contains(r#""kind": "sdkwork.component.spec""#));
    assert!(manifest_text.contains(r#""domain": "iot""#));
    assert!(manifest_text.contains(r#""type": "rust-crate""#));
    assert!(manifest_text.contains(r#""protocolPluginStandard""#));
    assert!(manifest_text.contains(r#""sdkwork_aiot_protocol::ProtocolAdapterManifest""#));
    assert!(manifest_text.contains(r#""codecs""#));
    assert!(manifest_text.contains(r#""session_policies""#));
    assert!(manifest_text.contains(r#""hardware_families""#));
    assert!(manifest_text.contains("API_SPEC.md"));
    assert!(manifest_text.contains("DATABASE_SPEC.md"));
    assert!(manifest_text.contains("COMPONENT_SPEC.md"));
    assert!(manifest_text.contains("DATABASE_FRAMEWORK_SPEC.md"));
    assert!(manifest_text.contains("IAM_MODULE_MANIFEST_SPEC.md"));
    assert!(manifest_text.contains("SDK_WORKSPACE_GENERATION_SPEC.md"));
    assert!(manifest_text.contains(r#""iamModuleManifest": "specs/iam.module.manifest.json""#));
    assert!(manifest_text.contains(r#""apis/app-api/iot/sdkwork-aiot-app-api.openapi.json""#));
}

#[test]
fn iam_module_manifest_is_present_and_valid() {
    let root = workspace_root();
    let manifest_path = root.join("specs/iam.module.manifest.json");
    assert!(
        manifest_path.exists(),
        "specs/iam.module.manifest.json is required for IMF federation"
    );
    let manifest_text = fs::read_to_string(&manifest_path).expect("iam module manifest");
    assert!(manifest_text.contains(r#""kind": "sdkwork.iam.module""#));
    assert!(manifest_text.contains(r#""moduleId": "iot""#));
    assert!(manifest_text.contains(r#""domain": "iot""#));
    assert!(manifest_text.contains("iot.devices.read"));
    assert!(manifest_text.contains("iot_platform_admin"));
}

#[test]
fn authored_crates_do_not_reimplement_crypto_primitives() {
    let root = workspace_root();
    let forbidden_patterns = ["fn sha256_digest", "const K: [u32; 64]"];
    let mut source_files = Vec::new();
    for dir in ["crates", "services"] {
        for entry in walkdir_files(root.join(dir)) {
            if entry
                .components()
                .any(|component| component.as_os_str() == "src")
                && entry.extension().is_some_and(|ext| ext == "rs")
            {
                source_files.push(entry);
            }
        }
    }

    for source_path in source_files {
        let source = fs::read_to_string(&source_path).expect("rust source");
        for pattern in forbidden_patterns {
            assert!(
                !source.contains(pattern),
                "{} must not reimplement crypto primitives; use sdkwork-utils-rust",
                source_path.display()
            );
        }
    }

    for crate_dir in [
        "crates/sdkwork-iot-platform-service",
        "crates/sdkwork-aiot-storage-sqlx",
        "crates/sdkwork-aiot-service-host",
    ] {
        let cargo =
            fs::read_to_string(root.join(crate_dir).join("Cargo.toml")).expect("Cargo.toml");
        assert!(
            cargo.contains("sdkwork-utils-rust"),
            "{crate_dir} must depend on sdkwork-utils-rust"
        );
    }
}

fn walkdir_files(dir: std::path::PathBuf) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() {
        return files;
    }
    for entry in fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            files.extend(walkdir_files(path));
        } else {
            files.push(path);
        }
    }
    files
}

#[test]
fn external_mqtt_broker_reference_is_rmqtt_only() {
    let root = workspace_root();
    let gitmodules = fs::read_to_string(root.join(".gitmodules")).expect(".gitmodules");

    assert!(
        gitmodules.contains(r#"[submodule "external/rmqtt"]"#),
        "rmqtt must be the canonical MQTT broker/server external implementation"
    );
    assert!(gitmodules.contains("https://github.com/rmqtt/rmqtt.git"));

    for removed in ["external/emqx", "external/mosquitto", "external/vernemq"] {
        assert!(
            !gitmodules.contains(removed),
            "{removed} must not remain as a MQTT broker external implementation"
        );
    }
}

#[test]
fn external_submodules_are_curated_high_signal_iot_references() {
    let root = workspace_root();
    let gitmodules = fs::read_to_string(root.join(".gitmodules")).expect(".gitmodules");

    let mut paths = gitmodules
        .lines()
        .filter_map(|line| line.trim().strip_prefix("path = "))
        .collect::<Vec<_>>();
    paths.sort_unstable();

    let mut expected = vec![
        "external/arduino-esp32",
        "external/esp-idf",
        "external/esphome",
        "external/micropython",
        "external/rmqtt",
        "external/tasmota",
        "external/thingsboard",
        "external/wled",
        "external/xiaozhi-esp32",
        "external/zephyr",
        "external/zigbee2mqtt",
    ];
    expected.sort_unstable();

    assert_eq!(
        paths, expected,
        "external submodules must stay focused on high-star smart-hardware references plus the explicit rmqtt MQTT implementation"
    );
}

#[test]
fn external_xiaozhi_esp32_application_declares_core_server_message_types() {
    let root = workspace_root();
    let application = fs::read_to_string(root.join("external/xiaozhi-esp32/main/application.cc"))
        .expect("external/xiaozhi-esp32 submodule must be initialized");
    let protocol =
        fs::read_to_string(root.join("external/xiaozhi-esp32/main/protocols/protocol.cc"))
            .expect("external/xiaozhi-esp32 protocol.cc must exist");

    for message_type in ["tts", "stt", "llm", "mcp", "system", "alert", "custom"] {
        assert!(
            application.contains(&format!("\"{message_type}\"")),
            "external/xiaozhi-esp32 application.cc must handle server message type {message_type}"
        );
    }

    for message_type in ["hello", "goodbye"] {
        let mqtt_protocol =
            fs::read_to_string(root.join("external/xiaozhi-esp32/main/protocols/mqtt_protocol.cc"))
                .expect("external/xiaozhi-esp32 mqtt_protocol.cc must exist");
        assert!(
            mqtt_protocol.contains(&format!("\"{message_type}\"")),
            "external/xiaozhi-esp32 mqtt_protocol.cc must reference transport message type {message_type}"
        );
    }

    for message_type in ["listen", "abort"] {
        assert!(
            protocol.contains(message_type),
            "external/xiaozhi-esp32 protocol.cc must reference device message type {message_type}"
        );
    }
}

#[test]
fn service_shells_read_topology_surface_bind_env_keys() {
    let root = workspace_root();
    let spec_text =
        fs::read_to_string(root.join("specs/topology.spec.json")).expect("topology spec");
    let retired_keys = topology_retired_env_keys(&spec_text);

    let cases = [
        (
            "services/sdkwork-aiot-cloud-gateway/src/main.rs",
            "SDKWORK_AIOT_EDGE_DEVICE_INGRESS_BIND",
        ),
        (
            "services/sdkwork-aiot-app-api/src/main.rs",
            "SDKWORK_AIOT_APPLICATION_APP_HTTP_BIND",
        ),
        (
            "services/sdkwork-aiot-admin-api/src/main.rs",
            "SDKWORK_AIOT_APPLICATION_ADMIN_HTTP_BIND",
        ),
        (
            "services/sdkwork-aiot-xiaozhi-simulator-ui/src/main.rs",
            "SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL",
        ),
    ];

    for (service, canonical_key) in cases {
        let source = fs::read_to_string(root.join(service)).expect(service);
        assert!(
            source.contains(canonical_key),
            "{service} must read canonical topology env key {canonical_key}"
        );
        for retired_key in &retired_keys {
            assert!(
                !source.contains(retired_key),
                "{service} must not read retired topology env key {retired_key}"
            );
        }
    }
}

#[test]
fn topology_dev_orchestrator_reads_spec_processes() {
    let root = workspace_root();
    let dev = fs::read_to_string(root.join("scripts/aiot-dev.mjs")).expect("aiot-dev orchestrator");

    assert!(
        dev.contains("listOrchestrationProcesses"),
        "scripts/aiot-dev.mjs must spawn processes from topology orchestration"
    );
    assert!(
        dev.contains("buildProcessEntries"),
        "scripts/aiot-dev.mjs must centralize process planning"
    );
    assert!(
        dev.contains("resolveDevProfileFromDeploymentProfile"),
        "scripts/aiot-dev.mjs must resolve profiles from deployment-profile axis"
    );
    assert!(
        dev.contains("--deployment-profile"),
        "scripts/aiot-dev.mjs must accept --deployment-profile"
    );
}

#[test]
fn pc_client_declares_topology_surface_env_keys() {
    let root = workspace_root();
    let topology_keys =
        root.join("apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-core/src/sdk/topologyEnvKeys.ts");
    let source = fs::read_to_string(&topology_keys).expect("aiot-pc-core topology env keys");

    for key in [
        "VITE_SDKWORK_AIOT_APPLICATION_APP_HTTP_URL",
        "VITE_SDKWORK_AIOT_APPLICATION_ADMIN_HTTP_URL",
        "VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL",
        "VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL",
    ] {
        assert!(
            source.contains(key),
            "aiot-pc-core must declare topology client env key {key}"
        );
    }

    assert!(
        root.join("apps/sdkwork-aiot-pc/.env.example").exists(),
        "apps/sdkwork-aiot-pc/.env.example is required"
    );
}

#[test]
fn sdk_families_have_openapi_sources_and_generation_manifests() {
    let root = workspace_root();

    for (family, authority_openapi, openapi_prefix, sdkgen_prefix, package_name) in [
        (
            "sdks/sdkwork-aiot-app-sdk",
            "apis/app-api/iot/sdkwork-aiot-app-api.openapi.json",
            "/app/v3/api/iot",
            "/app/v3/api",
            "@sdkwork/aiot-app-sdk",
        ),
        (
            "sdks/sdkwork-aiot-backend-sdk",
            "apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json",
            "/backend/v3/api/iot",
            "/backend/v3/api",
            "@sdkwork/aiot-backend-sdk",
        ),
    ] {
        let family_root = root.join(family);
        let openapi = root.join(authority_openapi);
        let sdkgen = family_root.join("openapi").join(format!(
            "{}.sdkgen.json",
            family_root.file_name().unwrap().to_string_lossy()
        ));
        let assembly = family_root.join(".sdkwork-assembly.json");

        let openapi_text = fs::read_to_string(&openapi).expect("openapi authority");
        let sdkgen_text = fs::read_to_string(&sdkgen).expect("sdkgen manifest");
        let assembly_text = fs::read_to_string(&assembly).expect("sdk assembly");

        assert!(openapi_text.contains(r#""openapi": "3.1.2""#));
        assert!(openapi_text.contains(openapi_prefix));
        assert!(openapi_text.contains(r#""AuthToken""#));
        assert!(openapi_text.contains(r#""AccessToken""#));
        assert!(openapi_text.contains(r#""name": "Access-Token""#));
        assert!(
            !openapi_text.contains(r#""SdkworkAccessToken""#),
            "{family} OpenAPI must not use retired SdkworkAccessToken security scheme"
        );
        assert!(
            !openapi_text.contains(r#""name": "Sdkwork-Access-Token""#),
            "{family} OpenAPI must not declare Sdkwork-Access-Token client header"
        );
        assert!(
            !openapi_text.contains(r#""name": "X-Sdkwork-Tenant-Id""#),
            "{family} OpenAPI must not expose client-writable tenant context headers"
        );
        assert!(
            !openapi_text.contains(r#""name": "X-Sdkwork-Organization-Id""#),
            "{family} OpenAPI must not expose client-writable organization context headers"
        );
        assert!(openapi_text.contains(r#""x-sdkwork-required-permission""#));
        assert!(openapi_text.contains("application/problem+json"));
        assert!(
            openapi_text.contains(r#""operationId": "devices.list""#)
                || openapi_text.contains(r#""operationId": "products.list""#),
            "{family} must expose resource-style dotted operationIds"
        );
        if family.ends_with("backend-sdk") {
            for expected in [
                r#""operationId": "protocolAdapters.list""#,
                r#""operationId": "runtime.capacity.retrieve""#,
                r#""x-sdkwork-required-permission": "iot.protocolAdapters.read""#,
                r#""x-sdkwork-required-permission": "iot.runtime.read""#,
                r#""AiotProtocolAdapter""#,
                r#""AiotRuntimeCapacityPolicy""#,
                r#""securityModes""#,
                r#""sessionPolicies""#,
                r#""hardwareFamilies""#,
                r#""backpressure""#,
            ] {
                assert!(
                    openapi_text.contains(expected),
                    "{family} OpenAPI missing {expected}"
                );
            }
        }
        assert!(sdkgen_text.contains(r#""standardProfile": "sdkwork-v3""#));
        assert!(sdkgen_text.contains(package_name));
        assert!(sdkgen_text.contains(sdkgen_prefix));
        assert!(sdkgen_text.contains("../../apis/"));
        assert!(assembly_text.contains(package_name));
        assert!(assembly_text.contains(r#""generatedProtocols": ["http"]"#));
        assert!(assembly_text.contains("../../apis/"));
    }
}

#[test]
fn typescript_sdk_boundaries_are_reserved_for_generated_clients() {
    let root = workspace_root();

    let app_package_root = root.join("sdks/sdkwork-aiot-app-sdk/sdkwork-aiot-app-sdk-typescript");
    let app_package_json = fs::read_to_string(app_package_root.join("package.json"))
        .expect("app typescript sdk package.json");
    let app_sdk_json = fs::read_to_string(app_package_root.join("sdkwork-sdk.json"))
        .expect("app typescript sdkwork-sdk.json");
    let app_index = fs::read_to_string(app_package_root.join("src").join("index.ts"))
        .expect("app typescript sdk index");

    assert!(app_package_json.contains("@sdkwork/aiot-app-sdk"));
    assert!(app_sdk_json.contains("@sdkwork/aiot-app-sdk"));
    assert!(app_sdk_json.contains(r#""generated": true"#));
    assert!(app_index.contains("createGeneratedAiotAppClient"));
    assert!(app_index.contains("generated/server-openapi"));
    assert!(app_index.contains("SdkworkAiotAppClient"));
    assert!(
        !app_index.contains("fetch(") && !app_index.contains("XMLHttpRequest"),
        "reserved app SDK boundary must not introduce handwritten transport logic"
    );

    let backend_package_root =
        root.join("sdks/sdkwork-aiot-backend-sdk/sdkwork-aiot-backend-sdk-typescript");
    let backend_package_json = fs::read_to_string(backend_package_root.join("package.json"))
        .expect("backend typescript sdk package.json");
    let backend_sdk_json = fs::read_to_string(backend_package_root.join("sdkwork-sdk.json"))
        .expect("backend typescript sdkwork-sdk.json");
    let backend_index = fs::read_to_string(backend_package_root.join("src").join("index.ts"))
        .expect("backend typescript sdk index");

    assert!(backend_package_json.contains("@sdkwork/aiot-backend-sdk"));
    assert!(backend_sdk_json.contains("@sdkwork/aiot-backend-sdk"));
    assert!(backend_sdk_json.contains(r#""generated": true"#));
    assert!(backend_index.contains("createGeneratedAiotBackendClient"));
    assert!(backend_index.contains("generated/server-openapi"));
    assert!(backend_index.contains("SdkworkAiotBackendClient"));
    assert!(
        !backend_index.contains("fetch(") && !backend_index.contains("XMLHttpRequest"),
        "reserved backend SDK boundary must not introduce handwritten transport logic"
    );
}

#[test]
fn http_api_route_contracts_are_reflected_in_openapi_sources() {
    let root = workspace_root();

    for route in standard_api_route_contracts() {
        let openapi_path = match route.surface {
            AiotApiSurface::App => "apis/app-api/iot/sdkwork-aiot-app-api.openapi.json",
            AiotApiSurface::Admin => "apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json",
        };
        let openapi = fs::read_to_string(root.join(openapi_path)).expect(openapi_path);

        assert!(
            openapi.contains(&format!(r#""{}""#, route.path)),
            "{openapi_path} missing path {}",
            route.path
        );
        assert!(
            openapi.contains(&format!(r#""operationId": "{}""#, route.operation_id)),
            "{openapi_path} missing operationId {}",
            route.operation_id
        );
        assert!(
            openapi.contains(&format!(
                r#""x-sdkwork-required-permission": "{}""#,
                route.required_permission
            )),
            "{openapi_path} missing required permission {} for {}",
            route.required_permission,
            route.operation_id
        );
    }
}

#[test]
fn openapi_operations_are_reflected_in_http_api_route_contracts() {
    let root = workspace_root();
    let contracts = standard_api_route_contracts();

    for (surface, openapi_path) in [
        (
            AiotApiSurface::App,
            "apis/app-api/iot/sdkwork-aiot-app-api.openapi.json",
        ),
        (
            AiotApiSurface::Admin,
            "apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json",
        ),
    ] {
        let openapi = fs::read_to_string(root.join(openapi_path)).expect(openapi_path);

        for operation_id in quoted_json_values_after_key(&openapi, "operationId") {
            assert!(
                contracts.iter().any(|route| {
                    route.surface == surface && route.operation_id == operation_id
                }),
                "{openapi_path} operationId {operation_id} missing from Rust route contracts"
            );
        }
    }
}

#[test]
fn openapi_operation_permissions_match_http_api_route_contracts() {
    let root = workspace_root();
    let contracts = standard_api_route_contracts();

    for route in contracts {
        let openapi_path = match route.surface {
            AiotApiSurface::App => "apis/app-api/iot/sdkwork-aiot-app-api.openapi.json",
            AiotApiSurface::Admin => "apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json",
        };
        let openapi = fs::read_to_string(root.join(openapi_path)).expect(openapi_path);
        let permission = openapi_permission_for_operation(&openapi, route.operation_id)
            .unwrap_or_else(|| {
                panic!(
                    "{openapi_path} missing permission for {}",
                    route.operation_id
                )
            });

        assert_eq!(
            permission, route.required_permission,
            "{openapi_path} permission mismatch for {}",
            route.operation_id
        );
    }
}

#[test]
fn backend_openapi_uses_media_resource_contract_for_firmware_artifact_io() {
    let root = workspace_root();
    let backend_openapi =
        fs::read_to_string(root.join("apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json"))
            .expect("backend openapi");

    assert!(backend_openapi.contains(r#""AiotFirmwareArtifactCreateRequest""#));
    assert!(backend_openapi.contains(r#""resource": {"#));
    assert!(backend_openapi.contains(r##""$ref": "#/components/schemas/MediaResource""##));
    assert!(backend_openapi.contains(r#""MediaKind""#));
    assert!(backend_openapi.contains(r#""MediaSource""#));
    assert!(backend_openapi.contains(r#""MediaAccess""#));
    assert!(backend_openapi.contains(r#""MediaChecksum""#));
    assert!(
        !backend_openapi.contains(r#""storageUri""#),
        "firmware artifact MediaResource contract must not expose bare storageUri fields"
    );
}

#[test]
fn event_openapi_contracts_use_typed_event_payload_and_media_resource_fields() {
    let root = workspace_root();
    let app_openapi =
        fs::read_to_string(root.join("apis/app-api/iot/sdkwork-aiot-app-api.openapi.json"))
            .expect("app openapi");
    let backend_openapi =
        fs::read_to_string(root.join("apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json"))
            .expect("backend openapi");

    assert!(app_openapi.contains(r#""AiotEvent""#));
    assert!(app_openapi.contains(r##""$ref": "#/components/schemas/AiotEvent""##));
    assert!(app_openapi.contains(r##""$ref": "#/components/schemas/SdkWorkListResponse""##));
    assert!(app_openapi.contains(r##""$ref": "#/components/schemas/MediaResource""##));
    assert!(!app_openapi.contains(r#""eventImageUrl""#));
    assert!(!app_openapi.contains(r#""eventAudioUrl""#));

    assert!(backend_openapi.contains(r#""AiotEvent""#));
    assert!(backend_openapi.contains(r##""$ref": "#/components/schemas/SdkWorkListResponse""##));
    assert!(backend_openapi.contains(r##""$ref": "#/components/schemas/MediaResource""##));
    assert!(!backend_openapi.contains(r#""eventImageUrl""#));
    assert!(!backend_openapi.contains(r#""eventAudioUrl""#));
}

#[test]
fn command_openapi_contracts_use_media_resource_for_request_and_result_payloads() {
    let root = workspace_root();
    let app_openapi =
        fs::read_to_string(root.join("apis/app-api/iot/sdkwork-aiot-app-api.openapi.json"))
            .expect("app openapi");
    let backend_openapi =
        fs::read_to_string(root.join("apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json"))
            .expect("backend openapi");

    assert!(app_openapi.contains(r#""AiotCommandCreateRequest""#));
    assert!(app_openapi.contains(r#""AiotCommandResult""#));
    assert!(app_openapi.contains(r##""$ref": "#/components/schemas/SdkWorkCommandResponse""##));
    assert!(app_openapi.contains(r#""requestMediaResourceId""#));
    assert!(app_openapi.contains(r#""resultMediaResourceId""#));
    assert!(app_openapi.contains(r##""$ref": "#/components/schemas/MediaResource""##));
    assert!(!app_openapi.contains(r#""requestAudioUrl""#));
    assert!(!app_openapi.contains(r#""resultAudioUrl""#));

    assert!(backend_openapi.contains(r#""AiotCommand""#));
    assert!(backend_openapi.contains(r#""AiotCommandResult""#));
    assert!(backend_openapi.contains(r##""$ref": "#/components/schemas/SdkWorkListResponse""##));
    assert!(backend_openapi.contains(r#""requestMediaResourceId""#));
    assert!(backend_openapi.contains(r#""resultMediaResourceId""#));
    assert!(backend_openapi.contains(r##""$ref": "#/components/schemas/MediaResource""##));
    assert!(!backend_openapi.contains(r#""requestAudioUrl""#));
    assert!(!backend_openapi.contains(r#""resultAudioUrl""#));
}

#[test]
fn declared_http_collection_routes_are_mounted_by_shared_api_component() {
    let http_api =
        fs::read_to_string(workspace_root().join("crates/sdkwork-iot-platform-service/src/lib.rs"))
            .expect("http api source");

    for route in standard_api_route_contracts() {
        if route.method == "GET"
            && !route.path.contains('{')
            && route.operation_id.ends_with(".list")
        {
            assert!(
                http_api.contains(route.path),
                "HTTP API component must mount declared collection route {}",
                route.path
            );
        }
    }
}

#[test]
fn crate_dependency_boundaries_do_not_invert_architecture() {
    let root = workspace_root();

    for crate_manifest in [
        "crates/sdkwork-aiot-contract/Cargo.toml",
        "crates/sdkwork-iot-device-service/Cargo.toml",
        "crates/sdkwork-aiot-protocol/Cargo.toml",
        "crates/sdkwork-aiot-service-host/Cargo.toml",
        "crates/sdkwork-aiot-storage/Cargo.toml",
        "crates/sdkwork-aiot-storage-sqlx/Cargo.toml",
        "crates/sdkwork-aiot-security/Cargo.toml",
        "crates/sdkwork-aiot-observability/Cargo.toml",
        "crates/sdkwork-aiot-adapter-xiaozhi/Cargo.toml",
        "crates/sdkwork-aiot-transport/Cargo.toml",
        "crates/sdkwork-iot-platform-service/Cargo.toml",
    ] {
        let manifest = fs::read_to_string(root.join(crate_manifest)).expect(crate_manifest);

        assert!(
            !manifest.contains("services/"),
            "{crate_manifest} must not depend on service binaries"
        );
        assert!(
            !manifest.contains("sdkwork-appbase"),
            "{crate_manifest} must not depend on appbase concrete IAM packages"
        );
    }

    let adapter_manifest =
        fs::read_to_string(root.join("crates/sdkwork-aiot-adapter-xiaozhi/Cargo.toml"))
            .expect("xiaozhi manifest");
    assert!(
        !adapter_manifest.contains("sdkwork-aiot-storage-sqlx")
            && !adapter_manifest.contains("sqlx"),
        "protocol adapters must not depend on storage implementations"
    );

    let transport_manifest =
        fs::read_to_string(root.join("crates/sdkwork-aiot-transport/Cargo.toml"))
            .expect("transport manifest");
    assert!(
        !transport_manifest.contains("sdkwork-aiot-adapter-xiaozhi"),
        "transport must stay protocol-neutral and accept codec/plugin injection"
    );
}

#[test]
fn protocol_plugin_manifest_standard_fields_are_not_eroded() {
    let root = workspace_root();
    let protocol_source = fs::read_to_string(root.join("crates/sdkwork-aiot-protocol/src/lib.rs"))
        .expect("protocol source");
    let xiaozhi_source =
        fs::read_to_string(root.join("crates/sdkwork-aiot-adapter-xiaozhi/src/lib.rs"))
            .expect("xiaozhi source");

    for expected in [
        "pub enum CodecKind",
        "pub enum SessionPolicy",
        "pub scope: ProtocolPluginScope",
        "pub codecs: Vec<CodecKind>",
        "pub session_policies: Vec<SessionPolicy>",
        "pub hardware_families: Vec<String>",
        "pub runtime_profiles: Vec<String>",
        "pub firmware_profiles: Vec<String>",
        "pub fn with_scope",
        "pub fn with_codec",
        "pub fn with_session_policy",
        "pub fn with_hardware_family",
    ] {
        assert!(
            protocol_source.contains(expected),
            "protocol manifest standard missing {expected}"
        );
    }

    for expected in [
        "with_scope(ProtocolPluginScope::CompatibilityPlugin)",
        "with_codec(CodecKind::JsonText)",
        "with_codec(CodecKind::JsonRpc)",
        "with_codec(CodecKind::BinaryMedia)",
        "with_session_policy(SessionPolicy::StatefulDeviceSession)",
        "with_hardware_family(\"esp32\")",
        "with_runtime_profile(\"esp_idf\")",
        "with_firmware_profile(\"xiaozhi_ota\")",
    ] {
        assert!(
            xiaozhi_source.contains(expected),
            "xiaozhi plugin manifest missing {expected}"
        );
    }
}

#[test]
fn committed_route_manifests_match_http_api_contracts() {
    let root = workspace_root();
    let manifest_specs = [
        (
            AiotApiSurface::App,
            "sdks/_route-manifests/app-api/sdkwork-aiot-app-api.route-manifest.json",
        ),
        (
            AiotApiSurface::Admin,
            "sdks/_route-manifests/backend-api/sdkwork-aiot-admin-api.route-manifest.json",
        ),
    ];

    for (surface, relative_path) in manifest_specs {
        let path = root.join(relative_path);
        let committed = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("missing route manifest {relative_path}: {error}");
        });
        let expected = sdkwork_iot_platform_service::standard_route_manifest_json(surface);
        assert_eq!(
            committed.trim(),
            expected.trim(),
            "route manifest drift detected for {relative_path}; run SDKWORK_EXPORT_ROUTE_MANIFESTS=1 cargo test -p sdkwork-iot-platform-service export_route_manifest_artifacts_when_requested -- --exact"
        );
    }

    for route in standard_api_route_contracts() {
        let relative_path = match route.surface {
            AiotApiSurface::App => {
                "sdks/_route-manifests/app-api/sdkwork-aiot-app-api.route-manifest.json"
            }
            AiotApiSurface::Admin => {
                "sdks/_route-manifests/backend-api/sdkwork-aiot-admin-api.route-manifest.json"
            }
        };
        let manifest = fs::read_to_string(root.join(relative_path)).expect(relative_path);
        assert!(
            manifest.contains(&format!(r#""operationId": "{}""#, route.operation_id)),
            "{relative_path} missing operationId {}",
            route.operation_id
        );
        assert!(
            manifest.contains(&format!(r#""path": "{}""#, route.path)),
            "{relative_path} missing path {}",
            route.path
        );
    }
}

#[test]
fn gateway_readiness_wires_outbox_and_optional_mqtt_bridge_probes() {
    let root = workspace_root();
    let gateway_lib =
        fs::read_to_string(root.join("services/sdkwork-aiot-cloud-gateway/src/lib.rs"))
            .expect("gateway lib");
    let gateway_main =
        fs::read_to_string(root.join("services/sdkwork-aiot-cloud-gateway/src/main.rs"))
            .expect("gateway main");
    let bridge_module =
        root.join("services/sdkwork-aiot-cloud-gateway/src/mqtt_bridge_readiness.rs");

    assert!(
        bridge_module.exists(),
        "gateway must expose mqtt bridge readiness module"
    );
    assert!(
        gateway_lib.contains("attach_gateway_readiness_probe"),
        "gateway lib must attach readiness probes"
    );
    assert!(
        gateway_lib.contains("mqtt_bridge_readiness_probe"),
        "gateway lib must chain mqtt bridge readiness"
    );
    assert!(
        gateway_main.contains("MqttBridgeRuntimeState::from_env"),
        "gateway main must bootstrap shared mqtt bridge runtime state"
    );
}

#[test]
fn service_host_supports_chained_readiness_probes() {
    let root = workspace_root();
    let source = fs::read_to_string(root.join("crates/sdkwork-aiot-service-host/src/lib.rs"))
        .expect("service-host lib");
    assert!(
        source.contains("fn and_readiness_probe"),
        "AiotHealthCheck must support chained readiness probes"
    );
}

#[test]
fn device_database_supports_postgres_device_repository() {
    let root = workspace_root();
    let device_database =
        fs::read_to_string(root.join("crates/sdkwork-aiot-storage-sqlx/src/device_database.rs"))
            .expect("device database");
    assert!(
        device_database
            .contains("SqliteSqlxDeviceRepository::from_blocking_pool(self.pool.clone())"),
        "device database must open device repository from BlockingDevicePool for all engines"
    );
    assert!(
        !device_database.contains("POSTGRES_DEVICE_PERSISTENCE_DEFERRED"),
        "device database must not defer Postgres device repository after Phase K"
    );
    let database_bootstrap =
        fs::read_to_string(root.join("crates/sdkwork-aiot-storage-sqlx/src/database_bootstrap.rs"))
            .expect("database bootstrap");
    assert!(
        !database_bootstrap.contains("POSTGRES_DEVICE_PERSISTENCE_DEFERRED"),
        "database bootstrap must not retain Phase K deferral markers"
    );
}

#[test]
fn production_topology_profiles_are_guarded() {
    let root = workspace_root();
    let package_json = fs::read_to_string(root.join("package.json")).expect("package.json");
    assert!(
        package_json.contains("\"check:production-topology\""),
        "package.json must expose production topology validation"
    );
    assert!(
        root.join("scripts/dev/validate-production-topology.mjs")
            .exists(),
        "production topology validator script is required"
    );
}

#[test]
fn xiaozhi_production_intelligence_bridge_is_declared() {
    let root = workspace_root();
    let workspace = fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml");
    assert!(
        workspace.contains("sdkwork-aiot-intelligence-bridge"),
        "workspace must include sdkwork-aiot-intelligence-bridge"
    );

    let topology_spec =
        fs::read_to_string(root.join("specs/topology.spec.json")).expect("topology spec");
    assert!(
        topology_spec.contains(r#""intelligence""#),
        "topology.spec.json must declare intelligence env keys"
    );
    assert!(
        topology_spec.contains("SDKWORK_AIOT_INTELLIGENCE_MODE"),
        "topology.spec.json must declare intelligence mode env key"
    );

    let adapter_cargo =
        fs::read_to_string(root.join("crates/sdkwork-aiot-adapter-xiaozhi/Cargo.toml"))
            .expect("adapter cargo");
    assert!(
        adapter_cargo.contains("audiopus"),
        "adapter-xiaozhi must depend on audiopus for Opus codec ownership"
    );

    let gateway_lib =
        fs::read_to_string(root.join("services/sdkwork-aiot-cloud-gateway/src/lib.rs"))
            .expect("gateway lib");
    assert!(
        gateway_lib.contains("mod xiaozhi_ws_media_session"),
        "gateway must track websocket uplink media sessions"
    );
    assert!(
        gateway_lib.contains("kernel_stack_from_env"),
        "gateway must wire kernel intelligence stack from env"
    );
}
