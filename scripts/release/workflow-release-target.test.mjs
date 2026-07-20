import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import { artifactPathFor, packageReleaseTarget, validateImageLock, validateReleaseTarget } from './workflow-release-target.mjs';

const root = path.resolve('.runtime', 'tests', 'aiot-workflow-target');

test.afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

test('packages a deterministic PC Web archive and binds its real digest', () => {
  const dist = path.join(root, 'apps', 'sdkwork-aiot-pc', 'dist');
  fs.mkdirSync(path.join(dist, 'assets'), { recursive: true });
  fs.writeFileSync(path.join(dist, 'index.html'), '<!doctype html>');
  fs.writeFileSync(path.join(dist, 'assets', 'app.js'), 'console.log("aiot")');
  const first = packageReleaseTarget({ packageId: 'web-universal-cloud-browser-zip', root, version: '0.1.0' });
  const firstBytes = fs.readFileSync(first.artifactPath);
  const second = packageReleaseTarget({ packageId: 'web-universal-cloud-browser-zip', root, version: '0.1.0' });
  assert.deepEqual(fs.readFileSync(second.artifactPath), firstBytes);
  assert.equal(first.artifactPath, artifactPathFor('web-universal-cloud-browser-zip', '0.1.0', root));
  assert.equal(first.manifest.clientArchitecture, 'pc-web');
  assert.doesNotThrow(() => validateReleaseTarget({ packageId: 'web-universal-cloud-browser-zip', root, version: '0.1.0' }));
});

test('packages the authored Mini Program output as a real non-empty archive', () => {
  const dist = path.join(root, 'apps', 'sdkwork-aiot-mini-program', 'dist');
  fs.mkdirSync(dist, { recursive: true });
  fs.writeFileSync(path.join(dist, 'app.json'), '{"pages":[]}');
  const result = packageReleaseTarget({ packageId: 'mp-weixin-universal-cloud-mini-program-mini-program-package', root, version: '0.1.0' });
  assert.ok(result.manifest.sizeBytes > 0);
  assert.equal(result.manifest.targetPlatform, 'mp-weixin');
});

test('cloud package validation requires an immutable OCI image digest', () => {
  fs.mkdirSync(root, { recursive: true });
  const lock = path.join(root, 'image-lock.json');
  fs.writeFileSync(lock, '{"image":"registry.sdkwork.com/sdkwork-aiot:latest"}');
  assert.throws(() => validateImageLock(lock), /immutable OCI/u);
  fs.writeFileSync(lock, `{"image":"registry.sdkwork.com/sdkwork-aiot@sha256:${'a'.repeat(64)}"}`);
  assert.doesNotThrow(() => validateImageLock(lock));
});
