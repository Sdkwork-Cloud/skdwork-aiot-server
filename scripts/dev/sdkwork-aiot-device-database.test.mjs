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

test('standalone topology declares the unified workspace PostgreSQL profile', () => {
  const env = parseEnv('etc/topology/standalone.development.env');

  assert.equal(env.get('SDKWORK_DATABASE_ENGINE'), 'postgresql');
  assert.equal(env.get('SDKWORK_DATABASE_NAME'), 'sdkwork_ai_dev');
  assert.equal(env.get('SDKWORK_DATABASE_SCHEMA'), 'sdkwork_ai_dev');
  assert.equal(env.get('SDKWORK_DATABASE_USERNAME'), 'sdkwork_ai_dev');
  assert.equal(env.get('SDKWORK_AIOT_OUTBOX_DISPATCHER_ENABLED'), '1');
  assert.equal(env.has('SDKWORK_DATABASE_URL'), false);
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

test('API assembly relies on the canonical database resolver', () => {
  const assembly = read('crates/sdkwork-api-aiot-assembly/src/bootstrap.rs');

  assert.match(assembly, /open_app_service_stores/u);
  assert.match(assembly, /open_admin_service_stores/u);
  assert.doesNotMatch(assembly, /SDKWORK_[A-Z0-9_]+_DATABASE_/u);
  assert.doesNotMatch(assembly, /APPLICATION_GATEWAY_DEVICE_DB_PATH/u);
});
