#!/usr/bin/env node
/**
 * Canonical SDKWork HTTP SDK generator entrypoint for sdkwork-aiot.
 * Prefers workspace ../sdkwork-sdk-generator/bin/sdkgen.js per SDK_SPEC.md.
 */
import { existsSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const generatorRoot = path.resolve(repoRoot, '../sdkwork-sdk-generator');
const canonicalBin = path.join(generatorRoot, 'bin', 'sdkgen.js');

function resolveSdkgenEntrypoint() {
  if (existsSync(canonicalBin)) {
    return { entrypoint: canonicalBin, useNode: true };
  }
  return { entrypoint: 'sdkgen', useNode: false };
}

const resolved = resolveSdkgenEntrypoint();
const args = process.argv.slice(2);
const spawnOptions = {
  stdio: 'inherit',
  shell: !resolved.useNode && process.platform === 'win32',
};

const result = resolved.useNode
  ? spawnSync(process.execPath, [resolved.entrypoint, ...args], spawnOptions)
  : spawnSync(resolved.entrypoint, args, spawnOptions);

if (result.error) {
  process.stderr.write(`[run-sdkgen] ${result.error.message}\n`);
  process.exit(1);
}

process.exit(result.status ?? 1);
