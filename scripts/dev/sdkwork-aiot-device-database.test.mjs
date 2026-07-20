#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

function parseEnv(relativePath) {
  return new Map(
    read(relativePath)
      .split(/\r?\n/u)
      .map((line) => line.trim())
      .filter((line) => line && !line.startsWith('#'))
      .map((line) => {
        const separator = line.indexOf('=');
        return [line.slice(0, separator), line.slice(separator + 1)];
      }),
  );
}

test('standalone topology declares one shared durable SQLite device database', () => {
  const env = parseEnv('etc/topology/standalone.development.env');

  assert.equal(env.get('SDKWORK_AIOT_DEVICE_DB_PATH'), '.sdkwork/dev/aiot-device.db');
  assert.equal(env.get('SDKWORK_AIOT_OUTBOX_DISPATCHER_ENABLED'), '1');
  assert.equal(env.has('SDKWORK_AIOT_DEVICE_DATABASE_URL'), false);
  assert.equal(env.has('SDKWORK_AIOT_APPLICATION_APP_HTTP_BIND'), false);
  assert.equal(env.has('SDKWORK_AIOT_APPLICATION_ADMIN_HTTP_BIND'), false);
});

test('device edge runtime uses the shared database environment resolver', () => {
  const edgeRuntime = read('crates/sdkwork-aiot-device-edge-runtime/src/lib.rs');

  assert.match(edgeRuntime, /open_aiot_device_database_from_env/u);
  assert.doesNotMatch(
    edgeRuntime,
    /device_credential_repository_from_env[\s\S]*ENV_DEVICE_DB_PATH\?\)/u,
  );
});

test('API assembly reads the same canonical device database path key', () => {
  const assembly = read('crates/sdkwork-api-aiot-assembly/src/bootstrap.rs');

  assert.match(assembly, /SDKWORK_AIOT_DEVICE_DB_PATH/u);
  assert.doesNotMatch(assembly, /APPLICATION_GATEWAY_DEVICE_DB_PATH/u);
});
