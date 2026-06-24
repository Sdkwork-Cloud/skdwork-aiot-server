#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

test('release package script resolves platform-specific release binaries', async () => {
  const moduleUrl = pathToFileURL(
    path.join(repoRoot, 'scripts/release-package.mjs'),
  ).href;
  const {
    RELEASE_PACKAGE_TARGETS,
    SERVER_BINARIES,
    resolveReleaseBinaryPath,
  } = await import(moduleUrl);

  assert.equal(SERVER_BINARIES.length, 3);
  assert.equal(RELEASE_PACKAGE_TARGETS.length, 2);
  assert.match(
    resolveReleaseBinaryPath(SERVER_BINARIES[0], 'win32'),
    /sdkwork-aiot-cloud-gateway\.exe$/u,
  );
  assert.match(
    resolveReleaseBinaryPath(SERVER_BINARIES[0], 'linux'),
    /sdkwork-aiot-cloud-gateway$/u,
  );
});

test('release package targets align with sdkwork.app.config.json package ids', () => {
  const releaseScript = fs.readFileSync(
    path.join(repoRoot, 'scripts/release-package.mjs'),
    'utf8',
  );
  const appConfig = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'sdkwork.app.config.json'), 'utf8'),
  );
  const packageIds = new Set(
    (appConfig.artifacts?.installConfig?.packages ?? []).map((pkg) => pkg.id),
  );

  for (const id of [
    'linux-x64-standalone-server-tar-gz',
    'windows-x64-standalone-server-zip',
  ]) {
    assert.ok(packageIds.has(id), `missing app config package id ${id}`);
    assert.match(releaseScript, new RegExp(id, 'u'));
  }

  const linuxPackage = (appConfig.artifacts?.installConfig?.packages ?? []).find(
    (pkg) => pkg.id === 'linux-x64-standalone-server-tar-gz',
  );
  assert.ok(linuxPackage?.url?.endsWith('linux/x64/server.tar.gz'));
  const windowsPackage = (appConfig.artifacts?.installConfig?.packages ?? []).find(
    (pkg) => pkg.id === 'windows-x64-standalone-server-zip',
  );
  assert.ok(windowsPackage?.url?.endsWith('windows/x64/server.zip'));
  assert.match(releaseScript, /linux\/x64\/server\.tar\.gz/u);
  assert.match(releaseScript, /windows\/x64\/server\.zip/u);
});
