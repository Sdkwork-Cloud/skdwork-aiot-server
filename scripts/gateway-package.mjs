#!/usr/bin/env node
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outputDir = path.join(root, 'artifacts/release');
mkdirSync(outputDir, { recursive: true });

const binaries = [
  { crate: 'sdkwork-aiot-cloud-gateway', windows: 'sdkwork-aiot-cloud-gateway.exe', unix: 'sdkwork-aiot-cloud-gateway' },
  { crate: 'sdkwork-aiot-app-api', windows: 'sdkwork-aiot-app-api.exe', unix: 'sdkwork-aiot-app-api' },
  { crate: 'sdkwork-aiot-admin-api', windows: 'sdkwork-aiot-admin-api.exe', unix: 'sdkwork-aiot-admin-api' },
];

for (const binary of binaries) {
  const fileName = process.platform === 'win32' ? binary.windows : binary.unix;
  const source = path.join(root, 'target/release', fileName);
  if (!existsSync(source)) {
    console.error(
      `[gateway:package] missing release binary ${source}; run pnpm gateway:build or pnpm release:build first`,
    );
    process.exit(1);
  }
  copyFileSync(source, path.join(outputDir, fileName));
  console.log(`[gateway:package] copied ${fileName}`);
}
