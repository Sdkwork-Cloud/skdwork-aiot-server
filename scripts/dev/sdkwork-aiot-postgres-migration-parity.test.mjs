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

test('sqlite and postgres admin-entity baselines stay paired', () => {
  for (const dialect of ['sqlite', 'postgres']) {
    const baselinePath = `database/ddl/baseline/${dialect}/0001_aiot_baseline.sql`;
    assert.ok(fs.existsSync(path.join(repoRoot, baselinePath)), baselinePath);

    const baseline = read(baselinePath);
    assert.match(baseline, /iot_admin_entity/u);
    assert.match(baseline, /entity_kind/u);
    assert.match(baseline, /iot_row_id_allocator/u);
    assert.match(baseline, /uk_iot_command_tenant_idempotency_key/u);
    assert.match(baseline, /uk_iot_device_credential_tenant_device_active/u);
  }
});

test('standalone production topology declares kernel intelligence env', () => {
  const envText = read('configs/topology/standalone.production.env');
  assert.match(envText, /SDKWORK_AIOT_INTELLIGENCE_MODE=kernel/u);
});

test('cloud production topology declares postgres device database env', () => {
  const envText = read('configs/topology/cloud.production.env');
  assert.match(envText, /SDKWORK_AIOT_DEVICE_DATABASE_ENGINE=postgres/u);
  assert.match(envText, /SDKWORK_AIOT_DEVICE_DATABASE_URL=/u);
  assert.match(envText, /SDKWORK_AIOT_INTELLIGENCE_MODE=kernel/u);
});

test('OpenAPI list contracts require PageInfo.mode', () => {
  for (const relativePath of [
    'apis/app-api/iot/sdkwork-aiot-app-api.openapi.json',
    'apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json',
  ]) {
    const openapi = JSON.parse(read(relativePath));
    const pageInfo = openapi.components?.schemas?.PageInfo;
    assert.ok(pageInfo, `${relativePath} must define PageInfo`);
    assert.ok(
      pageInfo.required?.includes('mode'),
      `${relativePath} PageInfo.mode must be required`,
    );
    assert.deepEqual(pageInfo.properties?.mode?.enum, ['offset', 'cursor']);
  }
});

test('backend credential response schema is typed', () => {
  const openapi = JSON.parse(read('apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json'));
  const schema = openapi.components?.schemas?.AiotCredentialResponse;
  assert.equal(schema?.type, 'object');
  assert.ok(schema.required?.includes('credentialId'));
  assert.ok(schema.properties?.issuedSecret);
});

test('embedded storage migrations stay aligned with baseline tables', () => {
  const libRs = read('crates/sdkwork-aiot-storage-sqlx/src/lib.rs');
  const sqliteBaseline = read('database/ddl/baseline/sqlite/0001_aiot_baseline.sql');
  for (const table of ['iot_device', 'iot_command', 'iot_outbox_event']) {
    assert.match(libRs, new RegExp(`CREATE TABLE ${table}`, 'u'));
    assert.match(sqliteBaseline, new RegExp(`CREATE TABLE ${table}`, 'u'));
  }
  assert.match(libRs, /CREATE TABLE IF NOT EXISTS iot_admin_entity/u);
  assert.match(sqliteBaseline, /CREATE TABLE IF NOT EXISTS iot_admin_entity/u);
  assert.match(libRs, /CREATE TABLE IF NOT EXISTS iot_row_id_allocator/u);
  assert.match(sqliteBaseline, /CREATE TABLE IF NOT EXISTS iot_row_id_allocator/u);
  assert.match(libRs, /uk_iot_device_credential_tenant_device_active/u);
  assert.match(sqliteBaseline, /uk_iot_device_credential_tenant_device_active/u);
});
