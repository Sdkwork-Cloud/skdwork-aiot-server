#!/usr/bin/env node
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const root = process.cwd();

function exists(relativePath) {
  return existsSync(path.join(root, relativePath));
}

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), 'utf8');
}

test('sdkwork-aiot uses the SDKWork standard project-root directory dictionary', () => {
  for (const directory of [
    'apis',
    'apps',
    'crates',
    'sdks',
    'jobs',
    'tools',
    'plugins',
    'examples',
    'configs',
    'deployments',
    'scripts',
    'docs',
    'tests',
  ]) {
    assert.ok(exists(`${directory}/README.md`), `${directory}/README.md must exist`);
  }
});

test('sdkwork-aiot keeps API authority inputs under apis', () => {
  for (const apiPath of [
    'apis/app-api/iot/sdkwork-aiot-app-api.openapi.json',
    'apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json',
  ]) {
    assert.ok(exists(apiPath), `${apiPath} must exist`);
  }
});

test('sdkwork-aiot root package.json follows PNPM script standard', () => {
  const result = spawnSync(
    'node',
    [
      '../sdkwork-specs/tools/check-pnpm-script-standard.mjs',
      '--root',
      '.',
      '--application-code-prefix',
      'aiot',
    ],
    { cwd: root, encoding: 'utf8' },
  );
  assert.equal(
    result.status,
    0,
    `pnpm script standard failed:\n${result.stdout}\n${result.stderr}`,
  );
});

test('sdkwork-aiot dev scripts use deployment-profile axis instead of retired hosting flags', () => {
  const packageJson = JSON.parse(read('package.json'));
  const devCommand = String(packageJson.scripts?.dev ?? '');
  assert.match(
    devCommand,
    /--deployment-profile\s+standalone/u,
    'dev must use --deployment-profile standalone',
  );
  assert.doesNotMatch(devCommand, /--hosting/u, 'dev must not use retired --hosting');
  assert.doesNotMatch(
    JSON.stringify(packageJson.scripts ?? {}),
    /"aiot:/u,
    'public root scripts must not use application-code prefix aiot:',
  );
});

test('sdkwork-aiot declares sdkwork framework dependencies in workflow manifest', () => {
  const workflow = read('sdkwork.workflow.json');
  for (const dependency of [
    'sdkwork-web-framework',
    'sdkwork-database',
    'sdkwork-utils',
    'sdkwork-sdk-generator',
    'sdkwork-app-topology',
  ]) {
    assert.match(workflow, new RegExp(`"id": "${dependency}"`, 'u'), `${dependency} dependency required`);
  }
});

test('sdkwork-aiot uses responsibility-specific Rust crate names', () => {
  for (const cratePath of [
    'crates/sdkwork-iot-device-service/Cargo.toml',
    'crates/sdkwork-aiot-service-host/Cargo.toml',
    'crates/sdkwork-iot-platform-service/Cargo.toml',
    'crates/sdkwork-router-iot-app-api/Cargo.toml',
    'crates/sdkwork-router-iot-backend-api/Cargo.toml',
  ]) {
    assert.ok(exists(cratePath), `${cratePath} must exist`);
  }

  const forbidden = [
    'crates/sdkwork-aiot-core',
    'crates/sdkwork-aiot-runtime',
    'crates/sdkwork-aiot-backend',
    'crates/sdkwork-aiot-common',
  ];
  for (const cratePath of forbidden) {
    assert.equal(exists(cratePath), false, `${cratePath} must not exist`);
  }
});

test('sdkwork-aiot dev orchestrator resolves deployment profiles through topology adapter', () => {
  const devScript = read('scripts/aiot-dev.mjs');
  assert.match(devScript, /resolveDevProfileFromDeploymentProfile/u);
  assert.match(devScript, /--deployment-profile/u);
  assert.match(
    devScript,
    /--hosting is retired/u,
    'aiot-dev must reject retired --hosting flag',
  );
});

test('sdkwork-aiot h5 core must not read live tokens from VITE env', () => {
  const h5CoreSource = read(
    'apps/sdkwork-aiot-h5/packages/sdkwork-aiot-h5-core/src/index.ts',
  );
  const h5SessionSource = read(
    'apps/sdkwork-aiot-h5/packages/sdkwork-aiot-h5-core/src/sdk/h5RuntimeSession.ts',
  );
  const h5AuthGateSource = read(
    'apps/sdkwork-aiot-h5/packages/sdkwork-aiot-h5-core/src/auth/AiotH5AuthGate.tsx',
  );
  const h5AppSource = read('apps/sdkwork-aiot-h5/src/App.tsx');
  for (const source of [h5CoreSource, h5SessionSource, h5AuthGateSource]) {
    assert.doesNotMatch(source, /VITE_SDKWORK_AUTH_TOKEN/u);
    assert.doesNotMatch(source, /VITE_SDKWORK_ACCESS_TOKEN/u);
  }
  assert.match(h5CoreSource, /AiotH5AuthGate/u);
  assert.match(h5AppSource, /<AiotH5AuthGate/u);
});

test('sdkwork-aiot firmware rollout OTA alignment artifacts are present', () => {
  for (const relativePath of [
    'database/migrations/sqlite/0002_aiot_admin_entity_schema.up.sql',
    'database/migrations/postgres/0002_aiot_admin_entity_schema.up.sql',
    'database/seeds/common/001_bootstrap.sql',
    'crates/sdkwork-aiot-storage-sqlx/src/firmware_ota_catalog.rs',
    'services/sdkwork-aiot-cloud-gateway/tests/gateway_standard.rs',
    'deployments/deploy.yaml',
    '.github/workflows/ci.yml',
  ]) {
    assert.ok(exists(relativePath), `missing rollout alignment artifact: ${relativePath}`);
  }

  const ciWorkflow = read('.github/workflows/ci.yml');
  assert.match(ciWorkflow, /release-smoke/u);

  const otaCatalog = read('crates/sdkwork-aiot-storage-sqlx/src/firmware_ota_catalog.rs');
  assert.match(otaCatalog, /DEPLOYMENT_STATE_PENDING/u);
  assert.match(otaCatalog, /mark_deployment_completed/u);
  assert.match(otaCatalog, /mark_offered_deployment_completed_for_device/u);

  const h5Core = read('apps/sdkwork-aiot-h5/packages/sdkwork-aiot-h5-core/src/index.ts');
  assert.match(h5Core, /setTokenManager/u);
  assert.match(h5Core, /getAiotH5TokenManager/u);

  const gatewayTests = read('services/sdkwork-aiot-cloud-gateway/tests/gateway_standard.rs');
  assert.match(
    gatewayTests,
    /rollout_aware_ota_provider_delivers_firmware_once_per_pending_deployment/u,
  );
  assert.match(
    gatewayTests,
    /xiaozhi_mqtt_session_reply_with_options_persists_protocol_storage_command/u,
  );

  const pcCore = read('apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-core/src/index.ts');
  assert.match(pcCore, /getAiotPcTokenManager/u);
  assert.match(read('apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-core/src/sdk/aiotAppSdkClient.ts'), /setTokenManager/u);

  const packageJson = JSON.parse(read('package.json'));
  assert.doesNotMatch(
    JSON.stringify(packageJson.scripts ?? {}),
    /cargo fmt --all/u,
    'check must format only sdkwork-aiot workspace members, not sibling path dependencies',
  );
  assert.match(packageJson.scripts?.check ?? '', /cargo fmt -- --check/u);
  assert.match(packageJson.scripts?.check ?? '', /check:docs-standard/u);

  const gatewayLib = read('services/sdkwork-aiot-cloud-gateway/src/lib.rs');
  const ingestFinalizers = gatewayLib.match(/finalize_protocol_ingest\(&result\.storage_command, &receipt\)/gu);
  assert.ok(
    ingestFinalizers && ingestFinalizers.length >= 2,
    'websocket and mqtt ingress must finalize protocol ingest consistently',
  );

  const releaseScript = read('scripts/release-package.mjs');
  assert.match(releaseScript, /windows\/x64\/server\.zip/u);
  assert.match(releaseScript, /resolveReleaseBinaryPath/u);

  const deployManifest = read('deployments/deploy.yaml');
  assert.match(deployManifest, /self-hosted\.split-services\.production/u);
  assert.match(deployManifest, /cloud-hosted\.split-services\.production/u);

  const releasePublish = read('scripts/release-publish.mjs');
  assert.match(releasePublish, /release:publish/u);
  assert.match(read('scripts/release-package.mjs'), /generateReleaseSboms/u);

  assert.ok(exists('docs/runbooks/production-release.md'), 'production release runbook must exist');

  const docsIndex = read('docs/INDEX.yaml');
  assert.match(docsIndex, /production-release-runbook/u);
  assert.match(docsIndex, /kind: sdkwork\.docs\.index/u);

  assert.ok(exists('scripts/release-preflight.mjs'), 'release preflight script must exist');
});

test('sdkwork-aiot production intelligence alignment artifacts are present', () => {
  for (const relativePath of [
    'crates/sdkwork-aiot-intelligence-bridge/Cargo.toml',
    'crates/sdkwork-aiot-intelligence-bridge/src/lib.rs',
    'crates/sdkwork-aiot-adapter-xiaozhi/src/opus_codec.rs',
    'crates/sdkwork-aiot-adapter-xiaozhi/src/opus_uplink.rs',
    'crates/sdkwork-aiot-adapter-xiaozhi/src/provider_downlink.rs',
    'services/sdkwork-aiot-cloud-gateway/src/xiaozhi_ws_media_session.rs',
    'docs/architecture/XIAOZHI_INTELLIGENCE_INTEGRATION.md',
  ]) {
    assert.ok(exists(relativePath), `missing intelligence alignment artifact: ${relativePath}`);
  }

  const topologySpec = JSON.parse(read('specs/topology.spec.json'));
  assert.ok(topologySpec.intelligence?.mode, 'topology.spec.json must declare intelligence.mode');
  assert.ok(
    topologySpec.intelligence?.kernelHttpUrl,
    'topology.spec.json must declare intelligence.kernelHttpUrl',
  );
  assert.ok(
    topologySpec.intelligence?.clawRouterOpenHttpUrl,
    'topology.spec.json must declare intelligence.clawRouterOpenHttpUrl',
  );

  const bridgeLib = read('crates/sdkwork-aiot-intelligence-bridge/src/speech.rs');
  assert.match(bridgeLib, /decode_xiaozhi_opus_uplink_to_wav/u);
  assert.match(bridgeLib, /asr_wav_bytes/u);

  const gatewayLib = read('services/sdkwork-aiot-cloud-gateway/src/lib.rs');
  assert.match(gatewayLib, /xiaozhi_ws_media_session/u);
  assert.match(gatewayLib, /push_ws_uplink_packet/u);
  assert.match(gatewayLib, /encode_provider_pcm_to_xiaozhi_opus_packets/u);

  const opusCodec = read('crates/sdkwork-aiot-adapter-xiaozhi/src/opus_codec.rs');
  assert.match(opusCodec, /audiopus/u);
  assert.match(read('crates/sdkwork-aiot-adapter-xiaozhi/Cargo.toml'), /audiopus/u);
});
