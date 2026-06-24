#!/usr/bin/env node
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

test('docs index registry paths resolve', () => {
  const result = spawnSync(
    process.execPath,
    [path.join(repoRoot, 'scripts/dev/validate-docs-index.mjs')],
    { cwd: repoRoot, encoding: 'utf8' },
  );
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /validate-docs-index\] ok/u);
});

test('release preflight script wires production gates', () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  );
  assert.match(
    packageJson.scripts?.['release:preflight'] ?? '',
    /release-preflight\.mjs/u,
  );
  const script = fs.readFileSync(
    path.join(repoRoot, 'scripts/release-preflight.mjs'),
    'utf8',
  );
  assert.match(script, /deploy:validate/u);
  assert.match(script, /release:validate/u);
  assert.match(script, /release:publish/u);
});
