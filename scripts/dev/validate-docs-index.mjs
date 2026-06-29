#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const indexPath = path.join(repoRoot, 'docs', 'INDEX.yaml');

function readIndexPaths(indexContent) {
  const paths = [];
  for (const match of indexContent.matchAll(/^\s+path:\s*(\S+)\s*$/gmu)) {
    paths.push(match[1]);
  }
  for (const match of indexContent.matchAll(
    /^\s+(?:prd|techArchitecture):\s*(\S+)\s*$/gmu,
  )) {
    paths.push(match[1]);
  }
  return paths;
}

function validateDocsIndex() {
  assert.ok(fs.existsSync(indexPath), 'docs/INDEX.yaml must exist');
  const indexContent = fs.readFileSync(indexPath, 'utf8');
  assert.match(indexContent, /kind:\s*sdkwork\.docs\.index/u);
  assert.match(indexContent, /canon:\s*\n\s+prd:\s*docs\/product\/prd\/PRD\.md/u);
  assert.match(
    indexContent,
    /techArchitecture:\s*docs\/architecture\/tech\/TECH_ARCHITECTURE\.md/u,
  );

  for (const relativePath of readIndexPaths(indexContent)) {
    const absolutePath = path.join(repoRoot, relativePath.replace(/\/$/u, ''));
    const exists = fs.existsSync(absolutePath);
    assert.ok(
      exists,
      `docs/INDEX.yaml path ${relativePath} must resolve to an existing file or directory`,
    );
  }
}

validateDocsIndex();
console.log('[validate-docs-index] ok');
