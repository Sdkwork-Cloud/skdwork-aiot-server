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

test('release preflight delegates to the workflow facade and stays draft fail-closed', () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  );
  assert.match(
    packageJson.scripts?.['release:preflight'] ?? '',
    /sdkwork-app release:preflight/u,
  );
  const workflow = JSON.parse(fs.readFileSync(path.join(repoRoot, 'sdkwork.workflow.json'), 'utf8'));
  assert.ok(workflow.lifecycle?.preflight?.length > 0);
  assert.equal(workflow.publish?.githubRelease, false);
  const thinWorkflow = fs.readFileSync(path.join(repoRoot, '.github/workflows/package.yml'), 'utf8');
  assert.match(thinWorkflow, /publish_release:\s*false/u);
  assert.doesNotMatch(thinWorkflow, /github\.event_name == 'release'/u);
});
