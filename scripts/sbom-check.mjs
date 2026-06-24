#!/usr/bin/env node
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  readAppConfig,
  resolveEnabledPackageIds,
  sbomOutputPath,
} from './sbom-generate.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const strict = process.argv.includes('--strict');

function readSbomChecksum(packageId) {
  const output = sbomOutputPath(packageId);
  const sbom = JSON.parse(readFileSync(output, 'utf8'));
  const properties = sbom.metadata?.properties ?? [];
  const checksumProperty = properties.find((entry) => entry.name === 'sdkwork:checksum');
  return checksumProperty?.value ?? null;
}

function main() {
  const appConfig = readAppConfig();
  if (!appConfig.security?.sbomRequired) {
    console.log('[sbom:check] skipped (sbomRequired=false)');
    return;
  }

  const packages = appConfig.artifacts?.installConfig?.packages ?? [];
  const packageIds = process.env.SDKWORK_PACKAGE_ID
    ? [process.env.SDKWORK_PACKAGE_ID]
    : resolveEnabledPackageIds(appConfig);
  let verified = 0;

  for (const packageId of packageIds) {
    const output = sbomOutputPath(packageId);
    if (!existsSync(output)) {
      if (strict) {
        console.error(
          `[sbom:check] missing SBOM evidence at ${path.relative(root, output)}`,
        );
        process.exit(1);
      }
      continue;
    }

    if (strict) {
      const pkg = packages.find((entry) => entry.id === packageId);
      const sbomChecksum = readSbomChecksum(packageId);
      assert.ok(
        typeof sbomChecksum === 'string' && sbomChecksum.length === 64,
        `${packageId} SBOM must record sdkwork:checksum`,
      );
      assert.equal(
        sbomChecksum,
        pkg?.checksum,
        `${packageId} SBOM checksum must match sdkwork.app.config.json`,
      );
    }

    verified += 1;
    console.log(`[sbom:check] ok ${path.relative(root, output)}`);
  }

  if (strict && verified === 0) {
    console.error('[sbom:check] no SBOM evidence found for enabled release packages');
    process.exit(1);
  }

  if (verified === 0) {
    console.log('[sbom:check] skipped (no SBOM files on disk; run pnpm sbom:generate)');
  }
}

const isMain =
  path.resolve(fileURLToPath(import.meta.url)) === path.resolve(process.argv[1] ?? '');
if (isMain) {
  main();
}
