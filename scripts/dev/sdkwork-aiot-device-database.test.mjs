#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

test('postgres dev orchestration wires SDKWORK_AIOT_DEVICE_DATABASE env', async () => {
  const moduleUrl = pathToFileURL(
    path.join(repoRoot, 'scripts/lib/aiot-device-database.mjs'),
  ).href;
  const {
    ENV_DEVICE_DATABASE_ENGINE,
    ENV_DEVICE_DATABASE_TABLE_PREFIX,
    ENV_DEVICE_DATABASE_URL,
    ENV_DEVICE_DB_PATH,
    mergeDeviceDatabaseEnv,
  } = await import(moduleUrl);

  const merged = mergeDeviceDatabaseEnv({}, { databaseEngine: 'postgres' });
  assert.equal(merged[ENV_DEVICE_DATABASE_ENGINE], 'postgres');
  assert.equal(merged[ENV_DEVICE_DATABASE_TABLE_PREFIX], 'iot_');
  assert.match(merged[ENV_DEVICE_DATABASE_URL], /^postgres:\/\//u);
  assert.equal(merged[ENV_DEVICE_DB_PATH], undefined);
});

test('dev orchestrator wires sqlite device db and outbox dispatch ownership', async () => {
  const moduleUrl = pathToFileURL(
    path.join(repoRoot, 'scripts/lib/aiot-device-database.mjs'),
  ).href;
  const {
    ENV_DEVICE_DB_PATH,
    ENV_DEVICE_DATABASE_ENGINE,
    ENV_DEVICE_DATABASE_URL,
    ENV_OUTBOX_DISPATCHER_ENABLED,
    mergeDeviceDatabaseEnv,
    mergeProcessRuntimeEnv,
    resolveDevDeviceDatabasePath,
  } = await import(moduleUrl);

  const sqlitePath = resolveDevDeviceDatabasePath('sqlite');
  assert.ok(sqlitePath);
  assert.match(sqlitePath, /[\\/]\.sdkwork[\\/]dev[\\/]aiot-device\.db/u);

  const merged = mergeDeviceDatabaseEnv({}, { databaseEngine: 'sqlite' });
  assert.equal(merged[ENV_DEVICE_DB_PATH], sqlitePath);
  assert.equal(merged[ENV_DEVICE_DATABASE_URL], undefined);
  assert.equal(merged[ENV_DEVICE_DATABASE_ENGINE], undefined);
  assert.ok(fs.existsSync(path.dirname(sqlitePath)));

  const edgeEnv = mergeProcessRuntimeEnv({ id: 'edge.device-ingress' }, merged);
  const appEnv = mergeProcessRuntimeEnv({ id: 'application.app-http' }, merged);
  const adminEnv = mergeProcessRuntimeEnv({ id: 'application.admin-http' }, merged);

  assert.equal(edgeEnv[ENV_OUTBOX_DISPATCHER_ENABLED], '1');
  assert.equal(appEnv[ENV_OUTBOX_DISPATCHER_ENABLED], '0');
  assert.equal(adminEnv[ENV_OUTBOX_DISPATCHER_ENABLED], '0');
});

test('aiot-dev applies device database env before spawning services', () => {
  const devScript = fs.readFileSync(path.join(repoRoot, 'scripts/aiot-dev.mjs'), 'utf8');
  assert.match(devScript, /mergeDeviceDatabaseEnv/u);
  assert.match(devScript, /mergeProcessRuntimeEnv/u);
  assert.match(devScript, /databaseEngine: settings\.database/u);
});
