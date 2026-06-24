#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

test('release publish script validates strict release evidence', () => {
  const publishScript = fs.readFileSync(
    path.join(repoRoot, 'scripts/release-publish.mjs'),
    'utf8',
  );
  assert.match(publishScript, /validate-release-artifacts\.mjs/u);
  assert.match(publishScript, /sbom-check\.mjs/u);
  assert.match(publishScript, /release:publish/u);
});

test('release package script generates SBOM evidence for enabled packages', () => {
  const releaseScript = fs.readFileSync(
    path.join(repoRoot, 'scripts/release-package.mjs'),
    'utf8',
  );
  assert.match(releaseScript, /generateReleaseSboms/u);
});
