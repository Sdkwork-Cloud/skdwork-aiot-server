#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const source = path.join(root, 'src');
const output = path.join(root, 'dist');

if (!fs.existsSync(path.join(source, 'app.json'))) {
  throw new Error('mini program source is missing src/app.json');
}

fs.rmSync(output, { recursive: true, force: true });
fs.cpSync(source, output, { recursive: true });
console.log(`[sdkwork-aiot-mini-program] built ${path.relative(root, output)}`);
