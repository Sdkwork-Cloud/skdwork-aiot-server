#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

const scanRoots = [
  'crates',
  'services',
  'scripts',
  'configs',
  'docs',
  'specs',
  'apps',
  'README.md',
  'AGENTS.md',
];

const skipPathFragments = [
  '/target/',
  '/node_modules/',
  '/external/',
  '/generated/',
  'sdkwork-aiot-topology-baggage.test.mjs',
  'docs/topology-standard.md',
  'docs/superpowers/',
  'scripts/aiot-dev.mjs',
];

const allowlistPathFragments = [
  'specs/topology.spec.json',
];

const bannedPatterns = [
  { id: 'local-minimal profile', pattern: /(?<![\w-])local-minimal(?![\w-])/u },
  { id: 'local-default profile', pattern: /(?<![\w-])local-default(?![\w-])/u },
  { id: 'topology v1 env key', pattern: /SDKWORK_AIOT_TOPOLOGY/u },
  { id: 'topology CLI flag', pattern: /--topology\b/u },
  { id: 'retired gateway bind env', pattern: /SDKWORK_AIOT_GATEWAY_BIND/u },
  { id: 'retired app api bind env', pattern: /SDKWORK_AIOT_APP_API_BIND/u },
  { id: 'retired admin api bind env', pattern: /SDKWORK_AIOT_ADMIN_API_BIND/u },
  {
    id: 'retired simulator gateway http env',
    pattern: /SDKWORK_AIOT_XIAOZHI_SIMULATOR_GATEWAY_HTTP/u,
  },
];

function slash(value) {
  return String(value).replaceAll('\\', '/');
}

function shouldSkip(relativePath) {
  const normalized = slash(relativePath);
  return skipPathFragments.some((fragment) => normalized.includes(fragment));
}

function isAllowlisted(relativePath) {
  const normalized = slash(relativePath);
  return allowlistPathFragments.some((fragment) => normalized.endsWith(fragment));
}

function collectFiles(relativeRoot) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) {
    return [];
  }
  const stat = fs.statSync(absoluteRoot);
  if (stat.isFile()) {
    return [relativeRoot];
  }
  const files = [];
  for (const entry of fs.readdirSync(absoluteRoot, { withFileTypes: true })) {
    const relativePath = path.join(relativeRoot, entry.name);
    if (shouldSkip(relativePath)) {
      continue;
    }
    if (entry.isDirectory()) {
      files.push(...collectFiles(relativePath));
      continue;
    }
    files.push(relativePath);
  }
  return files;
}

function readText(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8').replace(/^\uFEFF/u, '');
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
}

const files = scanRoots.flatMap((root) => collectFiles(root));

for (const { id, pattern } of bannedPatterns) {
  const hits = [];
  for (const relativePath of files) {
    if (isAllowlisted(relativePath)) {
      continue;
    }
    const text = readText(relativePath);
    if (pattern.test(text)) {
      hits.push(relativePath);
    }
  }
  assert.equal(
    hits.length,
    0,
    `topology baggage (${id}) found in active paths: ${hits.join(', ')}`,
  );
}

assert.ok(fs.existsSync(path.join(repoRoot, 'specs/topology.spec.json')), 'topology spec required');
const spec = readJson('specs/topology.spec.json');
assert.equal(spec.schemaVersion, 2);
assert.equal(spec.archetype, 'application-rest-edge-device');
assert.equal(spec.defaults.developmentProfileId, 'standalone.split-services.development');
assert.ok(spec.surfaces['application.app-http']);
assert.ok(spec.surfaces['application.admin-http']);
assert.ok(spec.surfaces['edge.device-ingress']);
assert.ok(spec.surfaces['platform.api-gateway']);

for (const [profileId, profilePath] of Object.entries(spec.profileFiles ?? {})) {
  assert.ok(fs.existsSync(path.join(repoRoot, profilePath)), `${profilePath} should exist for ${profileId}`);
  const profileEnv = readText(profilePath);
  assert.match(profileEnv, /SDKWORK_AIOT_PROFILE_ID=/u, `${profilePath} must set SDKWORK_AIOT_PROFILE_ID`);
  assert.match(profileEnv, /SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL=/u, `${profilePath} must set edge ingress HTTP URL`);
  assert.match(profileEnv, /VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL=/u, `${profilePath} must set client platform gateway URL`);
}

const profileDir = path.join(repoRoot, 'configs/topology');
const profileFiles = fs.readdirSync(profileDir).filter((name) => name.endsWith('.env'));
assert.ok(profileFiles.length >= 2, 'topology profile env files required');

const packageJson = JSON.parse(readText('package.json'));
assert.match(
  JSON.stringify(packageJson.dependencies ?? {}),
  /"@sdkwork\/app-topology"/u,
  'package.json must depend on @sdkwork/app-topology',
);
assert.match(
  JSON.stringify(packageJson.scripts?.dev ?? ''),
  /--deployment-profile\s+standalone/u,
  'package.json dev script must use --deployment-profile standalone',
);
assert.doesNotMatch(
  JSON.stringify(packageJson.scripts ?? {}),
  /"aiot:/u,
  'package.json must not expose application-code-prefixed public root scripts',
);

assert.equal(spec.scripts?.devOrchestrator, 'scripts/aiot-dev.mjs');

const aiotDevScript = readText('scripts/aiot-dev.mjs');
assert.match(aiotDevScript, /listOrchestrationProcesses/u, 'aiot-dev must spawn from topology orchestration');
assert.match(
  aiotDevScript,
  /--topology is retired/u,
  'aiot-dev must reject retired --topology flag',
);

const pcTopologyKeysPath =
  'apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-core/src/sdk/topologyEnvKeys.ts';
assert.ok(fs.existsSync(path.join(repoRoot, pcTopologyKeysPath)), 'aiot-pc-core topology env keys required');
const pcTopologyKeys = readText(pcTopologyKeysPath);
for (const clientKey of [
  'VITE_SDKWORK_AIOT_APPLICATION_APP_HTTP_URL',
  'VITE_SDKWORK_AIOT_APPLICATION_ADMIN_HTTP_URL',
  'VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL',
  'VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL',
]) {
  assert.match(
    pcTopologyKeys,
    new RegExp(clientKey, 'u'),
    `${pcTopologyKeysPath} must declare ${clientKey}`,
  );
}
assert.ok(
  fs.existsSync(path.join(repoRoot, 'apps/sdkwork-aiot-pc/.env.example')),
  'apps/sdkwork-aiot-pc/.env.example required',
);

const { loadProfile, resolveSurfaceHttpUrl } = await import(
  pathToFileURL(path.join(repoRoot, 'scripts/lib/aiot-topology.mjs')).href
);
const devProfileEnv = loadProfile('standalone.split-services.development');
assert.equal(
  resolveSurfaceHttpUrl(devProfileEnv, 'application.app-http'),
  'http://127.0.0.1:18082',
  'adapter should resolve application.app-http url from profile env',
);
assert.equal(
  resolveSurfaceHttpUrl(devProfileEnv, 'edge.device-ingress'),
  'http://127.0.0.1:18080',
  'adapter should resolve edge.device-ingress url from profile env',
);

assert.ok(fs.existsSync(path.join(repoRoot, 'scripts/aiot-dev.mjs')), 'aiot-dev orchestrator required');
assert.ok(fs.existsSync(path.join(repoRoot, 'scripts/lib/aiot-topology.mjs')), 'aiot topology adapter required');
assert.ok(fs.existsSync(path.join(repoRoot, 'docs/topology-standard.md')), 'topology-standard doc required');

console.log('[sdkwork-aiot-topology-baggage] ok');
