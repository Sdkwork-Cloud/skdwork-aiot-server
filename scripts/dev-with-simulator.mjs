#!/usr/bin/env node
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const child = spawn(
  process.execPath,
  [
    path.join(root, 'scripts/aiot-dev.mjs'),
    '--deployment-profile',
    'standalone',
    '--database',
    'sqlite',
    '--with-simulator',
    ...process.argv.slice(2),
  ],
  { cwd: root, stdio: 'inherit' },
);

child.on('exit', (code, signal) => {
  if (signal) {
    process.exit(1);
  }
  process.exit(code ?? 0);
});
