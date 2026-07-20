import assert from 'node:assert/strict';
import { generateKeyPairSync, verify } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import { createSbomAndProvenance, signReleaseArtifact } from './workflow-supply-chain-evidence.mjs';

const root = path.resolve('.runtime', 'tests', 'aiot-supply-chain');

test.afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

test('creates a verifiable signature plus byte-bound SBOM and provenance', () => {
  const relative = 'dist/release-packages/demo.zip';
  const artifact = path.join(root, relative);
  fs.mkdirSync(path.dirname(artifact), { recursive: true });
  const bytes = Buffer.from('immutable AIoT artifact');
  fs.writeFileSync(artifact, bytes);
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  const env = {
    SDKWORK_PACKAGE_ARTIFACT_PATH: relative,
    SDKWORK_PACKAGE_ID: 'demo-package',
    SDKWORK_PACKAGE_VERSION: '0.1.0',
    SDKWORK_RUNTIME_TARGET: 'server',
    SDKWORK_RELEASE_SIGNING_PRIVATE_KEY: privateKey.export({ format: 'pem', type: 'pkcs8' }).toString(),
  };
  const signed = signReleaseArtifact({ env, root });
  const envelope = JSON.parse(fs.readFileSync(signed.signaturePath, 'utf8'));
  assert.equal(verify(null, bytes, publicKey, Buffer.from(envelope.signatureBase64, 'base64')), true);
  const attested = createSbomAndProvenance({ env, root, sourceCommit: 'a'.repeat(40), evidenceWriter: () => {} });
  const sbom = JSON.parse(fs.readFileSync(attested.sbomPath, 'utf8'));
  const provenance = JSON.parse(fs.readFileSync(attested.provenancePath, 'utf8'));
  assert.equal(sbom.components[0].hashes[0].content, attested.digest.slice(7));
  assert.equal(provenance.subject[0].digest.sha256, attested.digest.slice(7));
});

test('refuses to create signing evidence without key material', () => {
  const relative = 'dist/release-packages/demo.zip';
  const artifact = path.join(root, relative);
  fs.mkdirSync(path.dirname(artifact), { recursive: true });
  fs.writeFileSync(artifact, 'artifact');
  assert.throws(() => signReleaseArtifact({ env: { SDKWORK_PACKAGE_ARTIFACT_PATH: relative, SDKWORK_PACKAGE_ID: 'demo' }, root }), /real private key/u);
});
