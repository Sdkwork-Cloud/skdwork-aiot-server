#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import { gzipSync } from 'node:zlib';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { createTopologyRuntime, loadTopologySpec } from '@sdkwork/app-topology';

import { createTar, createZip } from './archive.mjs';

const MODULE_PATH = fileURLToPath(import.meta.url);
const REPO_ROOT = path.resolve(path.dirname(MODULE_PATH), '..', '..');
const topologySpecPath = path.join(REPO_ROOT, 'specs', 'topology.spec.json');
const topologyRuntime = createTopologyRuntime(
  loadTopologySpec(topologySpecPath),
  REPO_ROOT,
  topologySpecPath,
);
const DEFINITIONS = Object.freeze({
  'web-universal-cloud-browser-zip': { kind: 'directory', archive: 'zip', source: 'apps/sdkwork-aiot-pc/dist', app: 'apps/sdkwork-aiot-pc', extension: 'zip', profileId: 'cloud.production', deploymentProfile: 'cloud', runtimeTarget: 'browser', targetPlatform: 'web', clientArchitecture: 'pc-web' },
  'h5-universal-cloud-mobile-zip': { kind: 'directory', archive: 'zip', source: 'apps/sdkwork-aiot-h5/dist', app: 'apps/sdkwork-aiot-h5', extension: 'zip', profileId: 'cloud.production', deploymentProfile: 'cloud', runtimeTarget: 'browser', targetPlatform: 'h5', clientArchitecture: 'h5' },
  'mp-weixin-universal-cloud-mini-program-mini-program-package': { kind: 'directory', archive: 'zip', source: 'apps/sdkwork-aiot-mini-program/dist', app: 'apps/sdkwork-aiot-mini-program', extension: 'mini-program-package', profileId: 'cloud.production', deploymentProfile: 'cloud', runtimeTarget: 'mini-program', targetPlatform: 'mp-weixin', clientArchitecture: 'mini-program' },
  'linux-x64-standalone-server-tar-gz': { kind: 'binary', archive: 'tar.gz', binary: 'sdkwork-api-aiot-standalone-gateway', extension: 'tar.gz', profileId: 'standalone.production', deploymentProfile: 'standalone', runtimeTarget: 'server' },
  'windows-x64-standalone-server-zip': { kind: 'binary', archive: 'zip', binary: 'sdkwork-api-aiot-standalone-gateway.exe', extension: 'zip', profileId: 'standalone.production', deploymentProfile: 'standalone', runtimeTarget: 'server' },
  'container-x64-cloud-container-kubernetes-tar-gz': { kind: 'container', archive: 'tar.gz', extension: 'tar.gz', profileId: 'cloud.production', deploymentProfile: 'cloud', runtimeTarget: 'container' },
});

function requireText(value, label) {
  const text = String(value ?? '').trim();
  if (!text) throw new Error(`${label} is required`);
  return text;
}

function definitionFor(packageId) {
  const definition = DEFINITIONS[packageId];
  if (!definition) throw new Error(`unsupported AIoT workflow target: ${packageId}`);
  return definition;
}

function releaseVersion(value) {
  const version = requireText(value, 'version').replace(/^v/u, '');
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u.test(version)) throw new Error(`invalid release version: ${value}`);
  return version;
}

export function artifactPathFor(packageId, version, root = REPO_ROOT) {
  const definition = definitionFor(packageId);
  return path.join(root, 'dist', 'release-packages', `sdkwork-aiot-${packageId}-${releaseVersion(version)}.${definition.extension}`);
}

export function createBuildPlan({ env = process.env, packageId, root = REPO_ROOT, version }) {
  const definition = definitionFor(packageId);
  const profileEnv = topologyRuntime.loadProfile(definition.profileId);
  const buildEnv = { ...env, ...profileEnv };
  const steps = [];
  if (definition.kind === 'directory') {
    steps.push({ command: process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm', args: ['run', '_sdkwork:build'], cwd: path.join(root, definition.app), env: buildEnv });
  } else if (definition.kind === 'binary') {
    steps.push({ command: process.platform === 'win32' ? 'cargo.exe' : 'cargo', args: ['build', '--release', '-p', 'sdkwork-api-aiot-standalone-gateway', '--bin', 'sdkwork-api-aiot-standalone-gateway'], cwd: root, env: buildEnv });
  } else {
    validateImageLock(requireText(env.SDKWORK_AIOT_CLOUD_IMAGE_LOCK_FILE, 'SDKWORK_AIOT_CLOUD_IMAGE_LOCK_FILE'));
  }
  return { definition, packageId, releaseVersion: releaseVersion(version), steps };
}

export function runBuildPlan(plan) {
  for (const step of plan.steps) {
    const result = spawnSync(step.command, step.args, { cwd: step.cwd, env: step.env, stdio: 'inherit', windowsHide: true });
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`build failed with exit code ${result.status ?? 1}`);
  }
}

export function packageReleaseTarget({ env = process.env, packageId, root = REPO_ROOT, version }) {
  const definition = definitionFor(packageId);
  const artifactPath = artifactPathFor(packageId, version, root);
  fs.mkdirSync(path.dirname(artifactPath), { recursive: true });
  let entries;
  if (definition.kind === 'directory') {
    entries = collectDirectory(path.join(root, definition.source));
  } else if (definition.kind === 'binary') {
    const source = path.join(root, 'target', 'release', definition.binary);
    if (!fs.existsSync(source)) throw new Error(`required release binary does not exist: ${source}`);
    entries = [{ relativePath: `bin/${definition.binary}`, data: fs.readFileSync(source), mode: 0o755 }];
  } else {
    const lockPath = requireText(env.SDKWORK_AIOT_CLOUD_IMAGE_LOCK_FILE, 'SDKWORK_AIOT_CLOUD_IMAGE_LOCK_FILE');
    validateImageLock(lockPath);
    entries = [
      fileEntry(lockPath, 'image-lock.json'),
      fileEntry(path.join(root, 'deployments', 'deploy.yaml'), 'deployments/deploy.yaml'),
      fileEntry(path.join(root, 'specs', 'topology.spec.json'), 'specs/topology.spec.json'),
      fileEntry(path.join(root, 'etc', 'topology', 'cloud.production.env'), 'etc/topology/cloud.production.env'),
    ];
  }
  const bytes = definition.archive === 'zip' ? createZip(entries) : gzipSync(createTar(entries), { mtime: 0 });
  if (bytes.length === 0) throw new Error(`${packageId} produced an empty artifact`);
  fs.writeFileSync(artifactPath, bytes);
  const manifest = {
    schemaVersion: 1,
    appId: 'sdkwork-aiot',
    packageId,
    version: releaseVersion(version),
    artifactPath: portable(path.relative(root, artifactPath)),
    sizeBytes: bytes.length,
    sha256: sha256(bytes),
    profileBinding: 'fixed',
    deploymentProfile: definition.deploymentProfile,
    runtimeTarget: definition.runtimeTarget,
    targetPlatform: definition.targetPlatform ?? null,
    clientArchitecture: definition.clientArchitecture ?? null,
  };
  const manifestPath = artifactPath.replace(new RegExp(`\\.${definition.extension.replace('.', '\\.')}\$`, 'u'), '.manifest.json');
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return { artifactPath, manifest, manifestPath };
}

export function validateReleaseTarget({ packageId, root = REPO_ROOT, version }) {
  const artifactPath = artifactPathFor(packageId, version, root);
  const definition = definitionFor(packageId);
  const manifestPath = artifactPath.replace(new RegExp(`\\.${definition.extension.replace('.', '\\.')}\$`, 'u'), '.manifest.json');
  if (!fs.existsSync(artifactPath) || !fs.existsSync(manifestPath)) throw new Error(`artifact or manifest is missing for ${packageId}`);
  const bytes = fs.readFileSync(artifactPath);
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  if (bytes.length === 0 || manifest.sha256 !== sha256(bytes) || manifest.packageId !== packageId) throw new Error(`${packageId} artifact validation failed`);
  return { artifactPath, manifestPath, sha256: manifest.sha256, sizeBytes: bytes.length };
}

function collectDirectory(root) {
  if (!fs.existsSync(root)) throw new Error(`required build output does not exist: ${root}`);
  const entries = [];
  for (const file of listFiles(root)) entries.push(fileEntry(file, portable(path.relative(root, file))));
  if (entries.length === 0) throw new Error(`build output is empty: ${root}`);
  return entries;
}

function listFiles(root) {
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(root, entry.name);
    return entry.isDirectory() ? listFiles(file) : entry.isFile() ? [file] : [];
  }).sort();
}

function fileEntry(source, relativePath) {
  if (!fs.existsSync(source)) throw new Error(`required package input does not exist: ${source}`);
  return { relativePath, data: fs.readFileSync(source), mode: 0o644 };
}

function validateImageLock(lockPath) {
  if (!fs.existsSync(lockPath)) throw new Error(`cloud image lock does not exist: ${lockPath}`);
  const content = fs.readFileSync(lockPath, 'utf8');
  if (!/@sha256:[a-f0-9]{64}/u.test(content)) throw new Error('cloud image lock must contain an immutable OCI @sha256 digest');
}

function sha256(value) { return createHash('sha256').update(value).digest('hex'); }
function portable(value) { return value.split(path.sep).join('/'); }

async function main(argv = process.argv.slice(2)) {
  const [command] = argv;
  const packageId = argv[argv.indexOf('--package-id') + 1];
  const version = argv[argv.indexOf('--version') + 1];
  if (command === 'build') runBuildPlan(createBuildPlan({ packageId, version }));
  else if (command === 'package') packageReleaseTarget({ packageId, version });
  else if (command === 'validate') validateReleaseTarget({ packageId, version });
  else throw new Error('command must be build, package, or validate');
}

if (process.argv[1] && path.resolve(process.argv[1]) === MODULE_PATH) {
  main().catch((error) => { console.error(`[sdkwork-aiot-workflow-target] ${error.message}`); process.exitCode = 1; });
}

export { DEFINITIONS, definitionFor, validateImageLock };
