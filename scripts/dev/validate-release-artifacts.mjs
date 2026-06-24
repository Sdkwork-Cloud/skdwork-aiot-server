#!/usr/bin/env node
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { createReadStream } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { pipeline } from 'node:stream/promises';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const strict = process.argv.includes('--strict');
const manifestPath = path.join(repoRoot, 'artifacts/release/release-packages.manifest.json');

async function sha256File(filePath) {
  const hash = createHash('sha256');
  await pipeline(createReadStream(filePath), hash);
  return hash.digest('hex');
}

async function main() {
  if (!fs.existsSync(manifestPath)) {
    if (strict) {
      throw new Error(
        'artifacts/release/release-packages.manifest.json is missing; run pnpm release:package first',
      );
    }
    console.log('[validate-release-artifacts] skipped (no release manifest)');
    return;
  }

  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const packages = manifest.packages ?? [];
  let validated = 0;

  for (const pkg of packages) {
    const archivePath = path.join(repoRoot, pkg.path);
    if (!fs.existsSync(archivePath)) {
      if (strict) {
        throw new Error(`missing release archive ${pkg.path}; run pnpm release:package first`);
      }
      continue;
    }

    const checksum = await sha256File(archivePath);
    assert.equal(
      checksum,
      pkg.checksum,
      `${pkg.path} checksum must match release-packages.manifest.json`,
    );
    validated += 1;
  }

  if (strict && validated === 0) {
    throw new Error('release manifest exists but no packaged archives were found on disk');
  }

  console.log(
    `[validate-release-artifacts] ok (${validated}/${packages.length} archives verified)`,
  );
}

await main();
