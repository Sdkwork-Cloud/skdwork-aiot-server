#!/usr/bin/env node
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const validator = path.join(repoRoot, 'scripts/dev/validate-deploy-manifest.mjs');

test('deploy manifest aligns production topology profiles', () => {
  const result = spawnSync(process.execPath, [validator], {
    cwd: repoRoot,
    encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /validate-deploy-manifest\] ok/u);
});

test('package.json wires deploy manifest validation', () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  );
  assert.match(packageJson.scripts?.['check:deploy-manifest'] ?? '', /validate-deploy-manifest/u);
  assert.match(packageJson.scripts?.['deploy:validate'] ?? '', /sdkwork-app deploy:validate/u);
  assert.match(packageJson.scripts?.['deploy:validate:standalone'] ?? '', /--profile standalone\.production/u);
  assert.match(packageJson.scripts?.['deploy:validate:cloud'] ?? '', /--profile cloud\.production/u);
});
