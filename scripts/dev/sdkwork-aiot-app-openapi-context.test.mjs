import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..', '..');

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(repoRoot, relativePath), 'utf8'));
}

function readText(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

const authorities = [
  'apis/app-api/iot/sdkwork-aiot-app-api.openapi.json',
  'apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json',
];

for (const relativePath of authorities) {
  const openapi = readJson(relativePath);
  const parameters = openapi.components?.parameters ?? {};
  for (const forbidden of [
    'SdkworkTenantId',
    'SdkworkOrganizationId',
    'SdkworkUserId',
    'SdkworkDataScope',
    'SdkworkPermissionScope',
  ]) {
    assert.equal(
      parameters[forbidden],
      undefined,
      `${relativePath} must not expose client-writable ${forbidden} request context parameters`,
    );
  }

  for (const [route, pathItem] of Object.entries(openapi.paths ?? {})) {
    if (!Array.isArray(pathItem?.parameters)) {
      continue;
    }
    for (const parameter of pathItem.parameters) {
      const ref = parameter?.$ref ?? '';
      assert.doesNotMatch(
        ref,
        /Sdkwork(?:Tenant|Organization|User|DataScope|PermissionScope)Id/u,
        `${relativePath} path ${route} must not reference client context header parameters`,
      );
    }
  }
}

const generatedIotApi = readText(
  'sdks/sdkwork-aiot-app-sdk/sdkwork-aiot-app-sdk-typescript/generated/server-openapi/src/api/iot.ts',
);
assert.doesNotMatch(
  generatedIotApi,
  /xSdkworkTenantId|X-Sdkwork-Tenant-Id/u,
  'generated AIoT app SDK must not require client tenant scope headers',
);
assert.match(
  generatedIotApi,
  /async list\(/u,
  'generated AIoT app SDK must expose token-scoped device list without tenant params',
);

console.log('sdkwork-aiot app OpenAPI context contract passed');
