#!/usr/bin/env node
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const appConfigPath = path.join(root, 'sdkwork.app.config.json');

export function resolveEnabledPackageIds(appConfig = readAppConfig()) {
  return (appConfig.artifacts?.installConfig?.packages ?? [])
    .filter((pkg) => pkg.enabled)
    .map((pkg) => pkg.id);
}

export function readAppConfig() {
  return JSON.parse(readFileSync(appConfigPath, 'utf8'));
}

export function sbomOutputPath(packageId) {
  return path.join(root, 'artifacts/release/sbom', `${packageId}.sbom.json`);
}

export function generateSbomForPackage(packageId, options = {}) {
  const appConfig = options.appConfig ?? readAppConfig();
  const pkg = (appConfig.artifacts?.installConfig?.packages ?? []).find(
    (entry) => entry.id === packageId,
  );
  const output = sbomOutputPath(packageId);
  mkdirSync(path.dirname(output), { recursive: true });
  writeFileSync(
    output,
    `${JSON.stringify(
      {
        bomFormat: 'CycloneDX',
        specVersion: '1.6',
        version: 1,
        metadata: {
          timestamp: new Date().toISOString(),
          tools: [{ vendor: 'SDKWork', name: 'sdkwork-aiot-sbom', version: '1.0.0' }],
          component: {
            type: 'application',
            name: process.env.SDKWORK_APP_ID || appConfig.appId || 'sdkwork-aiot',
            version:
              process.env.SDKWORK_PACKAGE_VERSION ||
              appConfig.release?.currentVersion ||
              '0.1.0',
            'bom-ref': packageId,
          },
          properties: [
            { name: 'sdkwork:packageId', value: packageId },
            {
              name: 'sdkwork:runtimeTarget',
              value: pkg?.runtimeTarget || process.env.SDKWORK_RUNTIME_TARGET || 'server',
            },
            {
              name: 'sdkwork:deploymentProfile',
              value:
                pkg?.deploymentProfile ||
                process.env.SDKWORK_DEPLOYMENT_PROFILE ||
                'standalone',
            },
            ...(pkg?.checksum
              ? [{ name: 'sdkwork:checksum', value: pkg.checksum }]
              : []),
          ],
        },
        components: [],
      },
      null,
      2,
    )}\n`,
    'utf8',
  );
  return output;
}

export function generateReleaseSboms(options = {}) {
  const appConfig = options.appConfig ?? readAppConfig();
  const packageIds =
    options.packageIds ??
    (process.env.SDKWORK_PACKAGE_ID
      ? [process.env.SDKWORK_PACKAGE_ID]
      : resolveEnabledPackageIds(appConfig));
  const outputs = [];
  for (const packageId of packageIds) {
    outputs.push(generateSbomForPackage(packageId, { appConfig }));
  }
  return outputs;
}

function main() {
  const outputs = generateReleaseSboms();
  for (const output of outputs) {
    console.log(`[sbom:generate] wrote ${path.relative(root, output)}`);
  }
}

const isMain =
  path.resolve(fileURLToPath(import.meta.url)) === path.resolve(process.argv[1] ?? '');
if (isMain) {
  main();
}
