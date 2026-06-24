#!/usr/bin/env node
import { existsSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

run('pnpm', ['deploy:validate']);
run('pnpm', ['release:validate']);

const manifestPath = path.join(root, 'artifacts/release/release-packages.manifest.json');
const linuxArchive = path.join(root, 'artifacts/release/linux/x64/server.tar.gz');
if (existsSync(manifestPath) && existsSync(linuxArchive)) {
  run('pnpm', ['release:publish']);
} else {
  console.log(
    '[release:preflight] skipped release:publish (no local release artifacts; run pnpm release:package first or rely on CI release-smoke)',
  );
}

console.log('[release:preflight] ok');
