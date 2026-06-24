#!/usr/bin/env node
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { sbomOutputPath } from './sbom-generate.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifestPath = path.join(root, 'artifacts/release/release-packages.manifest.json');
const appConfigPath = path.join(root, 'sdkwork.app.config.json');

function runNodeScript(relativePath, args = []) {
  const result = spawnSync(process.execPath, [path.join(root, relativePath), ...args], {
    cwd: root,
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    const output = `${result.stdout}\n${result.stderr}`.trim();
    throw new Error(`${relativePath} failed:\n${output}`);
  }
}

function main() {
  assert.ok(existsSync(manifestPath), 'release manifest missing; run pnpm release:package first');
  runNodeScript('scripts/dev/validate-release-artifacts.mjs', ['--strict']);
  runNodeScript('scripts/sbom-check.mjs', ['--strict']);

  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const appConfig = JSON.parse(readFileSync(appConfigPath, 'utf8'));
  const packages = appConfig.artifacts?.installConfig?.packages ?? [];

  console.log('[release:publish] release candidate is ready for CDN upload:');
  for (const released of manifest.packages ?? []) {
    const pkg = packages.find((entry) => entry.id === released.id);
    assert.ok(pkg?.enabled, `${released.id} must stay enabled in sdkwork.app.config.json`);
    assert.equal(pkg.checksum, released.checksum, `${released.id} checksum drift detected`);
    const sbomPath = sbomOutputPath(released.id);
    assert.ok(existsSync(sbomPath), `missing SBOM for ${released.id}`);
    console.log(`  archive: ${released.path}`);
    console.log(`    url: ${pkg.url}`);
    console.log(`    sha256: ${released.checksum}`);
    console.log(`    sbom: ${path.relative(root, sbomPath).replaceAll('\\', '/')}`);
  }

  console.log(
    '[release:publish] upload archives and SBOM files to the CDN URLs above, then promote via SDKWork packaging workflow.',
  );
}

const isMain =
  path.resolve(fileURLToPath(import.meta.url)) === path.resolve(process.argv[1] ?? '');
if (isMain) {
  main();
}
