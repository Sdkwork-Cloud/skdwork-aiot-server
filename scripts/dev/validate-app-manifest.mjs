#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const manifestPath = path.join(repoRoot, 'sdkwork.app.config.json');

function readManifest() {
  return JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
}

function isPlaceholderChecksum(checksum) {
  if (typeof checksum !== 'string' || checksum.length < 16) {
    return true;
  }
  const chunk = checksum.slice(0, 16);
  return checksum === chunk.repeat(Math.ceil(checksum.length / chunk.length)).slice(0, checksum.length);
}

function validateReleasePackages(manifest) {
  const packages = manifest.artifacts?.installConfig?.packages ?? [];
  const releaseManifestPath = path.join(repoRoot, 'artifacts/release/release-packages.manifest.json');
  let releaseManifest = null;
  if (fs.existsSync(releaseManifestPath)) {
    releaseManifest = JSON.parse(fs.readFileSync(releaseManifestPath, 'utf8'));
  }
  for (const pkg of packages) {
    if (!pkg.enabled) {
      continue;
    }
    assert.equal(
      pkg.checksumAlgorithm,
      'SHA-256',
      `${pkg.id} must declare SHA-256 checksum algorithm`,
    );
    assert.ok(
      typeof pkg.checksum === 'string' && pkg.checksum.length === 64,
      `${pkg.id} must declare a 64-char SHA-256 checksum`,
    );
    assert.ok(
      !isPlaceholderChecksum(pkg.checksum),
      `${pkg.id} must not use placeholder checksum values; disable the package until release:package publishes real artifacts`,
    );
    if (releaseManifest) {
      const released = releaseManifest.packages?.find((entry) => entry.id === pkg.id);
      if (released) {
        assert.equal(
          pkg.checksum,
          released.checksum,
          `${pkg.id} checksum must match artifacts/release/release-packages.manifest.json`,
        );
        const cdnSuffix = String(released.path)
          .replace(/^artifacts\/release\//u, '')
          .replaceAll('\\', '/');
        assert.ok(
          typeof pkg.url === 'string' && pkg.url.endsWith(cdnSuffix),
          `${pkg.id} url must end with ${cdnSuffix} to match packaged artifact path`,
        );
        if (manifest.security?.sbomRequired) {
          const sbomPath = path.join(
            repoRoot,
            'artifacts/release/sbom',
            `${pkg.id}.sbom.json`,
          );
          assert.ok(
            fs.existsSync(sbomPath),
            `${pkg.id} requires SBOM evidence at artifacts/release/sbom/${pkg.id}.sbom.json`,
          );
        }
      }
    }
  }
}

function validateDeploymentProfiles(manifest) {
  const profiles = manifest.runtime?.supportedDeploymentProfiles ?? [];
  assert.ok(
    profiles.includes('standalone'),
    'supportedDeploymentProfiles must include standalone',
  );
  assert.ok(
    profiles.includes('cloud'),
    'supportedDeploymentProfiles must include cloud for topology cloud-hosted profiles',
  );
}

function validateSecurity(manifest) {
  assert.equal(manifest.security?.checksumRequired, true, 'checksumRequired must stay enabled');
  assert.equal(manifest.security?.sbomRequired, true, 'sbomRequired must stay enabled');
}

const manifest = readManifest();
validateReleasePackages(manifest);
validateDeploymentProfiles(manifest);
validateSecurity(manifest);
console.log('sdkwork.app.config.json validation passed');
