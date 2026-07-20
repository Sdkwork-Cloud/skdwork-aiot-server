#!/usr/bin/env node

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const cli = process.env.WECHAT_DEVTOOLS_CLI?.trim();

if (!cli) {
  throw new Error('WECHAT_DEVTOOLS_CLI is required to start the WeChat Mini Program development surface');
}
if (!fs.existsSync(cli)) {
  throw new Error(`WECHAT_DEVTOOLS_CLI does not exist: ${cli}`);
}

const child = spawn(cli, ['open', '--project', root], {
  cwd: root,
  env: process.env,
  stdio: 'inherit',
  windowsHide: true,
});

child.once('error', (error) => {
  console.error(`[sdkwork-aiot-mini-program] ${error.message}`);
  process.exitCode = 1;
});
child.once('exit', (code) => {
  process.exitCode = code ?? 1;
});
