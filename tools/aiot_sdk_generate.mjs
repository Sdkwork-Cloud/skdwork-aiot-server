#!/usr/bin/env node
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HTTP_METHODS = new Set(['get', 'post', 'put', 'patch', 'delete']);
const workspaceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const families = [
  {
    familyName: 'sdkwork-aiot-app-sdk',
    authority: 'apis/app-api/iot/sdkwork-aiot-app-api.openapi.json',
    sdkgen: 'sdks/sdkwork-aiot-app-sdk/openapi/sdkwork-aiot-app-sdk.sdkgen.json',
    assembly: 'sdks/sdkwork-aiot-app-sdk/.sdkwork-assembly.json',
    routeManifest: 'sdks/_route-manifests/app-api/sdkwork-aiot-app-api.route-manifest.json',
    typescriptRoot: 'sdks/sdkwork-aiot-app-sdk/sdkwork-aiot-app-sdk-typescript',
    apiSurface: 'app-api',
    packageName: '@sdkwork/aiot-app-sdk',
    stripOpenApi: true,
  },
  {
    familyName: 'sdkwork-aiot-backend-sdk',
    authority: 'apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json',
    sdkgen: 'sdks/sdkwork-aiot-backend-sdk/openapi/sdkwork-aiot-backend-sdk.sdkgen.json',
    assembly: 'sdks/sdkwork-aiot-backend-sdk/.sdkwork-assembly.json',
    routeManifest:
      'sdks/_route-manifests/backend-api/sdkwork-aiot-admin-api.route-manifest.json',
    typescriptRoot: 'sdks/sdkwork-aiot-backend-sdk/sdkwork-aiot-backend-sdk-typescript',
    apiSurface: 'backend-api',
    packageName: '@sdkwork/aiot-backend-sdk',
    stripOpenApi: false,
  },
];

function fail(message) {
  process.stderr.write(`[aiot_sdk_generate] ${message}\n`);
  process.exit(1);
}

function resolveRoot(relativePath) {
  return path.resolve(workspaceRoot, relativePath);
}

function readJson(relativePath) {
  return JSON.parse(readFileSync(resolveRoot(relativePath), 'utf8'));
}

function readText(relativePath) {
  return readFileSync(resolveRoot(relativePath), 'utf8');
}

function collectOperationIds(openapi) {
  const operationIds = [];
  for (const pathItem of Object.values(openapi.paths ?? {})) {
    for (const [method, operation] of Object.entries(pathItem ?? {})) {
      if (!HTTP_METHODS.has(method) || !operation?.operationId) {
        continue;
      }
      operationIds.push(operation.operationId);
    }
  }
  return operationIds.sort();
}

function validateFamily(family) {
  for (const relativePath of [
    family.authority,
    family.sdkgen,
    family.assembly,
    family.routeManifest,
  ]) {
    if (!existsSync(resolveRoot(relativePath))) {
      throw new Error(`missing required SDK artifact: ${relativePath}`);
    }
  }

  const openapi = readJson(family.authority);
  if (openapi.openapi !== '3.1.2') {
    throw new Error(`${family.authority} must use OpenAPI 3.1.2`);
  }

  for (const pathItem of Object.values(openapi.paths ?? {})) {
    for (const operation of Object.values(pathItem ?? {})) {
      if (!operation || typeof operation !== 'object' || !operation.operationId) {
        continue;
      }
      if (operation['x-sdkwork-request-context'] !== 'WebRequestContext') {
        throw new Error(
          `${family.authority} operation ${operation.operationId} must declare WebRequestContext`,
        );
      }
      if (operation['x-sdkwork-api-surface'] !== family.apiSurface) {
        throw new Error(
          `${family.authority} operation ${operation.operationId} must declare apiSurface=${family.apiSurface}`,
        );
      }
    }
  }

  const sdkgen = readJson(family.sdkgen);
  if (sdkgen.standardProfile !== 'sdkwork-v3') {
    throw new Error(`${family.sdkgen} must use standardProfile sdkwork-v3`);
  }
  if (!String(sdkgen.authoritySpec ?? '').includes('apis/')) {
    throw new Error(`${family.sdkgen} must reference apis/ authority input`);
  }
  if (sdkgen.packageName !== family.packageName) {
    throw new Error(`${family.sdkgen} packageName mismatch for ${family.familyName}`);
  }

  const assembly = readJson(family.assembly);
  const assemblyText = readText(family.assembly);
  if (!assemblyText.includes(family.authority.replace(/^apis\//, '../../apis/'))) {
    throw new Error(`${family.assembly} must reference ${family.authority}`);
  }
  const generatedProtocols =
    assembly.generatedProtocols ?? assembly.discoverySurface?.generatedProtocols;
  if (!Array.isArray(generatedProtocols) || !generatedProtocols.includes('http')) {
    throw new Error(`${family.assembly} must declare generatedProtocols http`);
  }

  const routeManifest = readText(family.routeManifest);
  if (!routeManifest.includes('"requestContext": "WebRequestContext"')) {
    throw new Error(`${family.routeManifest} must declare WebRequestContext`);
  }
  if (!routeManifest.includes(`"apiSurface": "${family.apiSurface}"`)) {
    throw new Error(`${family.routeManifest} must declare apiSurface=${family.apiSurface}`);
  }

  const typescriptRoot = resolveRoot(family.typescriptRoot);
  for (const relativePath of ['package.json', 'src/index.ts', 'sdkwork-sdk.json']) {
    if (!existsSync(path.join(typescriptRoot, relativePath))) {
      throw new Error(`${family.typescriptRoot}/${relativePath} is required`);
    }
  }

  const sdkBoundary = readJson(path.join(family.typescriptRoot, 'sdkwork-sdk.json'));
  if (sdkBoundary.generated !== true) {
    throw new Error(`${family.typescriptRoot}/sdkwork-sdk.json must declare generated=true`);
  }

  const boundaryIndex = readText(path.join(family.typescriptRoot, 'src/index.ts'));
  if (boundaryIndex.includes('fetch(') || boundaryIndex.includes('XMLHttpRequest')) {
    throw new Error(`${family.typescriptRoot} must not introduce handwritten transport`);
  }

  const operationIds = collectOperationIds(openapi);
  if (operationIds.length === 0) {
    throw new Error(`${family.authority} must declare operations`);
  }
}

function runGenerate(family) {
  const materialize = spawnSync(
    'node',
    ['scripts/dev/sync-openapi-web-context.mjs'],
    { cwd: workspaceRoot, stdio: 'inherit' },
  );
  if (materialize.status !== 0) {
    throw new Error('sync-openapi-web-context materialization failed');
  }

  if (family.stripOpenApi) {
    const strip = spawnSync(
      'node',
      ['scripts/strip-openapi-context-headers.mjs'],
      { cwd: workspaceRoot, stdio: 'inherit' },
    );
    if (strip.status !== 0) {
      throw new Error('strip-openapi-context-headers failed');
    }
  }

  const generate = spawnSync('pnpm', ['run', 'generate'], {
    cwd: resolveRoot(family.typescriptRoot),
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });
  if (generate.status !== 0) {
    throw new Error(`sdk generation failed for ${family.familyName}`);
  }
}

function main() {
  const check = process.argv.includes('--check');
  try {
    for (const family of families) {
      validateFamily(family);
      if (!check) {
        runGenerate(family);
      }
    }
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }

  process.stdout.write(
    `[aiot_sdk_generate] ${check ? 'check passed' : 'generation completed'} for ${families.length} SDK families\n`,
  );
}

main();
