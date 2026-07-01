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
  }
});

test('cloud production topology declares postgres device database env', () => {
  const envText = read('configs/topology/cloud.split-services.production.env');
  assert.match(envText, /SDKWORK_AIOT_DEVICE_DATABASE_ENGINE=postgres/u);
  assert.match(envText, /SDKWORK_AIOT_DEVICE_DATABASE_URL=/u);
});
