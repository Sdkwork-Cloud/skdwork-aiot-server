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

test('sqlite and postgres admin-entity migrations stay paired', () => {
  for (const dialect of ['sqlite', 'postgres']) {
    const upPath = `database/migrations/${dialect}/0002_aiot_admin_entity_schema.up.sql`;
    const downPath = `database/migrations/${dialect}/0002_aiot_admin_entity_schema.down.sql`;
    assert.ok(fs.existsSync(path.join(repoRoot, upPath)), upPath);
    assert.ok(fs.existsSync(path.join(repoRoot, downPath)), downPath);

    const up = read(upPath);
    const down = read(downPath);
    assert.match(up, /iot_admin_entity/u);
    assert.match(up, /entity_kind/u);
    assert.match(down, /iot_admin_entity/u);
  }
});

test('cloud production topology declares postgres device database env', () => {
  const envText = read('configs/topology/cloud.split-services.production.env');
  assert.match(envText, /SDKWORK_AIOT_DEVICE_DATABASE_ENGINE=postgres/u);
  assert.match(envText, /SDKWORK_AIOT_DEVICE_DATABASE_URL=/u);
});
