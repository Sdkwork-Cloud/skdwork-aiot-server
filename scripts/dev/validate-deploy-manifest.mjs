#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const deployPath = path.join(repoRoot, 'deployments', 'deploy.yaml');
const topologyDir = path.join(repoRoot, 'configs', 'topology');

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
    values.set(trimmed.slice(0, separator).trim(), trimmed.slice(separator + 1).trim());
  }
  return values;
}

function extractDeployProfiles(deployContent) {
  const profiles = new Map();
  const lines = deployContent.split(/\r?\n/u);
  let currentId = null;
  let currentLines = [];

  for (const line of lines) {
    const profileMatch = line.match(/^  ([A-Za-z0-9._-]+):\s*$/u);
    if (profileMatch) {
      if (currentId) {
        profiles.set(currentId, currentLines.join('\n'));
      }
      currentId = profileMatch[1];
      currentLines = [];
      continue;
    }
    if (currentId) {
      if (/^\S/u.test(line)) {
        break;
      }
      currentLines.push(line);
    }
  }
  if (currentId) {
    profiles.set(currentId, currentLines.join('\n'));
  }
  return profiles;
}

function readDefaultProfile(deployContent) {
  const match = deployContent.match(/^defaultProfile:\s*([A-Za-z0-9._-]+)\s*$/mu);
  return match?.[1] ?? null;
}

function validateDeployManifest() {
  const deployContent = fs.readFileSync(deployPath, 'utf8');
  const profiles = extractDeployProfiles(deployContent);
  const defaultProfile = readDefaultProfile(deployContent);

  assert.ok(defaultProfile, 'deployments/deploy.yaml must declare defaultProfile');
  assert.ok(
    profiles.has(defaultProfile),
    `defaultProfile ${defaultProfile} must exist under profiles`,
  );

  const productionEnvFiles = fs
    .readdirSync(topologyDir)
    .filter((fileName) => fileName.endsWith('.production.env'))
    .sort();

  assert.ok(productionEnvFiles.length > 0, 'at least one production topology profile is required');

  for (const fileName of productionEnvFiles) {
    const values = parseEnvFile(fs.readFileSync(path.join(topologyDir, fileName), 'utf8'));
    const profileId = values.get('SDKWORK_AIOT_PROFILE_ID');
    assert.ok(profileId, `${fileName} must declare SDKWORK_AIOT_PROFILE_ID`);
    assert.ok(
      profiles.has(profileId),
      `deployments/deploy.yaml must declare profile ${profileId} for ${fileName}`,
    );

    const body = profiles.get(profileId);
    assert.match(
      body,
      /layout:\s*binary-package/u,
      `${profileId} production profile must use binary-package install layout`,
    );
    assert.match(
      body,
      /packages:\s*\n\s*-\s*server/u,
      `${profileId} production profile must install the server package`,
    );
  }
}

validateDeployManifest();
console.log('[validate-deploy-manifest] ok');
