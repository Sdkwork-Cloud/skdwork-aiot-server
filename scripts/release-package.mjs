#!/usr/bin/env node
import { createHash } from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { pipeline } from 'node:stream/promises';
import { generateReleaseSboms } from './sbom-generate.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const releaseDir = path.join(root, 'artifacts/release');
const manifestPath = path.join(releaseDir, 'release-packages.manifest.json');

export const SERVER_BINARIES = [
  {
    crate: 'sdkwork-api-aiot-standalone-gateway',
    unix: 'sdkwork-api-aiot-standalone-gateway',
    windows: 'sdkwork-api-aiot-standalone-gateway.exe',
  },
  {
    crate: 'sdkwork-aiot-device-edge-runtime',
    unix: 'sdkwork-aiot-device-edge-runtime',
    windows: 'sdkwork-aiot-device-edge-runtime.exe',
  },
];

export const RELEASE_PACKAGE_TARGETS = [
  {
    id: 'linux-x64-standalone-server-tar-gz',
    archiveRelativePath: 'linux/x64/server.tar.gz',
    bundleDir: 'bundle-linux',
    bundleFileName: (binary) => binary.unix,
    createArchive: createTarGz,
  },
  {
    id: 'windows-x64-standalone-server-zip',
    archiveRelativePath: 'windows/x64/server.zip',
    bundleDir: 'bundle-windows',
    bundleFileName: (binary) => binary.windows,
    createArchive: createZipArchive,
  },
];

export function resolveReleaseBinaryPath(binary, platform = process.platform) {
  const fileName = platform === 'win32' ? binary.windows : binary.unix;
  return path.join(root, 'target/release', fileName);
}

async function sha256File(filePath) {
  const hash = createHash('sha256');
  await pipeline(createReadStream(filePath), hash);
  return hash.digest('hex');
}

async function createTarGz(sourceDir, outputPath) {
  const result = spawnSync('tar', ['-czf', outputPath, '-C', sourceDir, '.'], {
    cwd: root,
    stdio: 'inherit',
  });
  if (result.status !== 0) {
    throw new Error(`tar failed with exit code ${result.status}`);
  }
}

async function createZipArchive(sourceDir, outputPath) {
  if (process.platform === 'win32') {
    const sourceGlob = path.join(sourceDir, '*');
    const result = spawnSync(
      'powershell',
      [
        '-NoProfile',
        '-Command',
        `Compress-Archive -Path '${sourceGlob}' -DestinationPath '${outputPath}' -Force`,
      ],
      { cwd: root, stdio: 'inherit' },
    );
    if (result.status !== 0) {
      throw new Error(`Compress-Archive failed with exit code ${result.status}`);
    }
    return;
  }

  const result = spawnSync('zip', ['-qr', outputPath, '.'], {
    cwd: sourceDir,
    stdio: 'inherit',
  });
  if (result.status !== 0) {
    throw new Error(`zip failed with exit code ${result.status}`);
  }
}

function stageBundle(bundleDir, bundleFileName) {
  mkdirSync(bundleDir, { recursive: true });
  for (const binary of SERVER_BINARIES) {
    const source = resolveReleaseBinaryPath(binary);
    if (!existsSync(source)) {
      console.error(
        `[release:package] missing release binary ${source}; run pnpm release:build first`,
      );
      process.exit(1);
    }
    const target = path.join(bundleDir, bundleFileName(binary));
    writeFileSync(target, readFileSync(source));
  }
}

async function packageReleaseTargets() {
  mkdirSync(releaseDir, { recursive: true });
  const packages = [];

  for (const target of RELEASE_PACKAGE_TARGETS) {
    const bundleDir = path.join(releaseDir, target.bundleDir);
    rmSync(bundleDir, { recursive: true, force: true });
    stageBundle(bundleDir, target.bundleFileName);

    const archivePath = path.join(releaseDir, target.archiveRelativePath);
    mkdirSync(path.dirname(archivePath), { recursive: true });
    rmSync(archivePath, { force: true });
    await target.createArchive(bundleDir, archivePath);
    const checksum = await sha256File(archivePath);
    packages.push({
      id: target.id,
      path: path.relative(root, archivePath).replaceAll('\\', '/'),
      checksumAlgorithm: 'SHA-256',
      checksum,
      enabled: true,
    });
    console.log(
      `[release:package] wrote ${path.relative(root, archivePath)} checksum=${checksum}`,
    );
  }

  return packages;
}

async function main() {
  const packages = await packageReleaseTargets();
  const manifest = {
    schemaVersion: 1,
    kind: 'sdkwork.release.packages',
    generatedAt: new Date().toISOString(),
    packages,
  };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  const sbomOutputs = generateReleaseSboms();
  for (const sbomPath of sbomOutputs) {
    console.log(`[release:package] wrote ${path.relative(root, sbomPath)}`);
  }
  console.log('[release:package] legacy archive output is isolated from sdkwork.app.config.json');
}

const isMain =
  path.resolve(fileURLToPath(import.meta.url)) === path.resolve(process.argv[1] ?? '');
if (isMain) {
  await main();
}
