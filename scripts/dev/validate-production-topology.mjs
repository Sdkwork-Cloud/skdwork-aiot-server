#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const topologyDir = path.join(repoRoot, 'configs', 'topology');

const MIN_SECRET_LENGTH = 32;
const FORBIDDEN_LEGACY_DEVICE_TOKEN = 'device-token';
const DEPLOY_INJECT_PREFIX = 'DEPLOY_INJECT:';

function parseEnvFile(content) {
  const values = new Map();
  for (const line of content.split(/\r?\n/u)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) {
      continue;
    }
    const separator = trimmed.indexOf('=');
    if (separator <= 0) {
      continue;
    }
    const key = trimmed.slice(0, separator).trim();
    const value = trimmed.slice(separator + 1).trim();
    values.set(key, value);
  }
  return values;
}

function requireDeployInjectPlaceholder(fileName, values, key) {
  const value = values.get(key);
  assert.ok(
    typeof value === 'string' && value.length >= MIN_SECRET_LENGTH,
    `${fileName} must set ${key} with at least ${MIN_SECRET_LENGTH} characters`,
  );
  assert.ok(
    value.startsWith(DEPLOY_INJECT_PREFIX),
    `${fileName} must set ${key} to a DEPLOY_INJECT: placeholder — inject the live secret at deploy time`,
  );
}

function resolveHosting(values) {
  const deploymentProfile = values.get('SDKWORK_AIOT_DEPLOYMENT_PROFILE');
  if (deploymentProfile === 'cloud') {
    return 'cloud-hosted';
  }
  if (deploymentProfile === 'standalone') {
    return 'self-hosted';
  }
  return values.get('SDKWORK_AIOT_HOSTING');
}

function validatePersistence(fileName, values) {
  const hosting = resolveHosting(values);
  const devicePath = values.get('SDKWORK_AIOT_DEVICE_DB_PATH');
  const databaseUrl = values.get('SDKWORK_AIOT_DEVICE_DATABASE_URL');
  const databaseEngine = values.get('SDKWORK_AIOT_DEVICE_DATABASE_ENGINE');

  if (hosting === 'cloud-hosted') {
    assert.ok(
      typeof databaseUrl === 'string' && databaseUrl.length > 0,
      `${fileName} must set SDKWORK_AIOT_DEVICE_DATABASE_URL for cloud Postgres persistence`,
    );
    assert.ok(
      databaseUrl.includes('DEPLOY_INJECT'),
      `${fileName} must use DEPLOY_INJECT database credential placeholders in tracked production topology`,
    );
    assert.equal(
      databaseEngine,
      'postgres',
      `${fileName} must set SDKWORK_AIOT_DEVICE_DATABASE_ENGINE=postgres for cloud-hosted production`,
    );
    return;
  }

  assert.ok(
    typeof devicePath === 'string' && devicePath.length > 0,
    `${fileName} must set SDKWORK_AIOT_DEVICE_DB_PATH for durable SQLite persistence`,
  );
}

function validateIntelligenceWhenEnabled(fileName, values) {
  const mode = values.get('SDKWORK_AIOT_INTELLIGENCE_MODE');
  if (mode !== 'kernel' && mode !== 'production') {
    return;
  }
  for (const key of [
    'SDKWORK_AIOT_INTELLIGENCE_KERNEL_HTTP_URL',
    'SDKWORK_CLAW_ROUTER_APPLICATION_OPEN_HTTP_URL',
  ]) {
    const value = values.get(key);
    assert.ok(
      typeof value === 'string' && value.length >= 8,
      `${fileName} must set ${key} when SDKWORK_AIOT_INTELLIGENCE_MODE=${mode}`,
    );
  }
  requireDeployInjectPlaceholder(fileName, values, 'SDKWORK_CLAW_ROUTER_API_KEY');
}

function validateProductionProfile(fileName, values) {
  assert.equal(
    values.get('SDKWORK_AIOT_ENVIRONMENT'),
    'production',
    `${fileName} must declare production environment`,
  );
  assert.notEqual(
    values.get('SDKWORK_AIOT_DEV_MODE'),
    '1',
    `${fileName} must not enable SDKWORK_AIOT_DEV_MODE in production`,
  );
  validatePersistence(fileName, values);
  assert.equal(
    values.get('SDKWORK_AIOT_OUTBOX_DISPATCHER_ENABLED'),
    '1',
    `${fileName} must enable outbox dispatch on edge gateway`,
  );

  requireDeployInjectPlaceholder(fileName, values, 'SDKWORK_AIOT_INTERNAL_TOKEN');
  requireDeployInjectPlaceholder(fileName, values, 'SDKWORK_AIOT_CREDENTIAL_PEPPER');

  const corsOrigins = values.get('SDKWORK_AIOT_CORS_ALLOWED_ORIGINS');
  assert.ok(
    typeof corsOrigins === 'string' && corsOrigins.length > 0,
    `${fileName} must set SDKWORK_AIOT_CORS_ALLOWED_ORIGINS`,
  );

  const legacyDeviceToken = values.get('SDKWORK_AIOT_XIAOZHI_DEVICE_TOKEN');
  assert.ok(
    legacyDeviceToken === undefined,
    `${fileName} must not set SDKWORK_AIOT_XIAOZHI_DEVICE_TOKEN in production`,
  );
  assert.notEqual(
    values.get('SDKWORK_AIOT_INTERNAL_TOKEN'),
    FORBIDDEN_LEGACY_DEVICE_TOKEN,
    `${fileName} must not use the default dev internal token`,
  );
  validateIntelligenceWhenEnabled(fileName, values);

  assert.equal(
    values.get('SDKWORK_AIOT_XIAOZHI_MCP_POLICY_DENY_BY_DEFAULT'),
    '1',
    `${fileName} must enable MCP deny-by-default in production`,
  );
}

const productionProfiles = fs
  .readdirSync(topologyDir)
  .filter((fileName) => fileName.endsWith('.production.env'));

assert.ok(productionProfiles.length >= 2, 'expected self-hosted and cloud production profiles');

for (const fileName of productionProfiles) {
  const content = fs.readFileSync(path.join(topologyDir, fileName), 'utf8');
  validateProductionProfile(fileName, parseEnvFile(content));
}

console.log(
  `[validate-production-topology] ok (${productionProfiles.length} production profiles)`,
);
