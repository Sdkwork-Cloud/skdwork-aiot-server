import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const rootDir = process.cwd();
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(rootDir, relativePath);
  assert.ok(fs.existsSync(absolutePath), `${relativePath} must exist`);
  return fs.readFileSync(absolutePath, 'utf8');
}

function fail(message) {
  failures.push(message);
}

const workflow = JSON.parse(read('sdkwork.workflow.json'));
const dependencyIds = new Set((workflow.dependencies || []).map((dependency) => dependency.id));
if (!dependencyIds.has('sdkwork-drive')) {
  fail('sdkwork.workflow.json must declare sdkwork-drive dependency');
}

const pnpmWorkspace = read('pnpm-workspace.yaml');
if (!pnpmWorkspace.includes('../sdkwork-drive/sdks/sdkwork-drive-app-sdk/sdkwork-drive-app-sdk-typescript')) {
  fail('pnpm-workspace.yaml must include sdkwork-drive-app-sdk package');
}

const driveClient = read('apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-core/src/sdk/driveAppSdkClient.ts');
if (!driveClient.includes('createDriveAppClient')) {
  fail('pc-core driveAppSdkClient must create @sdkwork/drive-app-sdk client');
}
if (!driveClient.includes('getDriveAppSdkClient')) {
  fail('pc-core driveAppSdkClient must expose getDriveAppSdkClient');
}

const backendClient = read('apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-core/src/sdk/aiotBackendSdkClient.ts');
if (!backendClient.includes('createAiotBackendClient')) {
  fail('pc-core aiotBackendSdkClient must create @sdkwork/aiot-backend-sdk client');
}

const firmwareUpload = read('apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-core/src/services/firmwareUploadService.ts');
if (!firmwareUpload.includes('uploadAiotFirmwareArtifactToDrive')) {
  fail('firmwareUploadService must expose uploadAiotFirmwareArtifactToDrive');
}
if (!firmwareUpload.includes('uploader.uploadArchive')) {
  fail('firmwareUploadService must route uploads through Drive uploader client');
}
if (!firmwareUpload.includes("source: 'drive'")) {
  fail('firmwareUploadService must emit Drive-backed MediaResource payloads');
}

const firmwarePanel = read(
  'apps/sdkwork-aiot-pc/packages/sdkwork-aiot-pc-console-iot/src/components/FirmwareArtifactUploadPanel.tsx',
);
if (!firmwarePanel.includes('uploadAiotFirmwareArtifactToDrive') && !firmwarePanel.includes('uploadArtifact')) {
  fail('FirmwareArtifactUploadPanel must upload firmware through Drive-backed service');
}

const backendOpenapi = read('apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json');
if (!backendOpenapi.includes('"drive"')) {
  fail('backend OpenAPI MediaSource must include drive');
}

const packageJson = JSON.parse(read('package.json'));
if (!packageJson.scripts?.['check:drive-standard']) {
  fail('package.json must expose check:drive-standard script');
}

if (failures.length > 0) {
  process.stderr.write(`Drive standard failed:\n${failures.map((failure) => `- ${failure}`).join('\n')}\n`);
  process.exit(1);
}

process.stdout.write('Drive standard passed\n');
