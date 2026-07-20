#!/usr/bin/env node

import { createHash, createPrivateKey, createPublicKey, sign as signBytes } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const MODULE_PATH = fileURLToPath(import.meta.url);
const REPO_ROOT = path.resolve(path.dirname(MODULE_PATH), '..', '..');

function required(value, label) {
  const text = String(value ?? '').trim();
  if (!text) throw new Error(`${label} is required`);
  return text;
}

function pathsFor(env = process.env, root = REPO_ROOT) {
  const packageId = required(env.SDKWORK_PACKAGE_ID, 'SDKWORK_PACKAGE_ID');
  const relative = required(env.SDKWORK_PACKAGE_ARTIFACT_PATH, 'SDKWORK_PACKAGE_ARTIFACT_PATH');
  if (path.isAbsolute(relative) || relative.split(/[\\/]/u).includes('..')) throw new Error('artifact path must be repository-relative');
  const artifactPath = path.resolve(root, relative);
  if (!fs.existsSync(artifactPath)) throw new Error(`artifact does not exist: ${artifactPath}`);
  const evidenceRoot = path.join(root, 'dist', 'release-evidence', packageId);
  const artifactName = path.basename(artifactPath);
  return {
    packageId,
    artifactPath,
    artifactRelativePath: portable(relative),
    evidenceRoot,
    signaturePath: path.join(evidenceRoot, `${artifactName}.sig`),
    sbomPath: path.join(evidenceRoot, `${artifactName}.cdx.json`),
    provenancePath: path.join(evidenceRoot, `${artifactName}.intoto.jsonl`),
  };
}

function signingKey(env = process.env) {
  const inline = String(env.SDKWORK_RELEASE_SIGNING_PRIVATE_KEY ?? '').trim();
  const file = String(env.SDKWORK_RELEASE_SIGNING_KEY_FILE ?? '').trim();
  if (!inline && !file) throw new Error('release signing requires a real private key');
  if (inline && file) throw new Error('configure exactly one release signing key source');
  if (file && !fs.existsSync(file)) throw new Error(`signing key file does not exist: ${file}`);
  return createPrivateKey({ key: inline || fs.readFileSync(file), passphrase: String(env.SDKWORK_RELEASE_SIGNING_PRIVATE_KEY_PASSWORD ?? '') || undefined });
}

export function signReleaseArtifact({ env = process.env, root = REPO_ROOT } = {}) {
  const paths = pathsFor(env, root);
  const bytes = fs.readFileSync(paths.artifactPath);
  const privateKey = signingKey(env);
  const algorithm = ['ed25519', 'ed448'].includes(privateKey.asymmetricKeyType) ? null : 'sha256';
  const signature = signBytes(algorithm, bytes, privateKey);
  const publicKey = createPublicKey(privateKey).export({ format: 'der', type: 'spki' });
  const envelope = {
    schemaVersion: 1,
    algorithm: privateKey.asymmetricKeyType,
    hashAlgorithm: algorithm ?? 'none',
    artifact: paths.artifactRelativePath,
    digest: `sha256:${sha256(bytes)}`,
    publicKeyFingerprint: `sha256:${sha256(publicKey)}`,
    signatureBase64: signature.toString('base64'),
  };
  fs.mkdirSync(paths.evidenceRoot, { recursive: true });
  fs.writeFileSync(paths.signaturePath, `${JSON.stringify(envelope, null, 2)}\n`, { mode: 0o600 });
  return { ...paths, envelope };
}

export function createSbomAndProvenance({ env = process.env, root = REPO_ROOT, sourceCommit = gitHead(root), evidenceWriter = writeWorkflowEvidence } = {}) {
  const paths = pathsFor(env, root);
  if (!fs.existsSync(paths.signaturePath)) throw new Error(`detached signature is missing: ${paths.signaturePath}`);
  const bytes = fs.readFileSync(paths.artifactPath);
  const digest = sha256(bytes);
  const version = required(env.SDKWORK_PACKAGE_VERSION, 'SDKWORK_PACKAGE_VERSION');
  const sbom = {
    bomFormat: 'CycloneDX',
    specVersion: '1.5',
    version: 1,
    metadata: { component: { type: 'application', name: paths.packageId, version, hashes: [{ alg: 'SHA-256', content: digest }] } },
    components: [{ type: 'file', name: path.basename(paths.artifactPath), version, hashes: [{ alg: 'SHA-256', content: digest }], properties: [{ name: 'sdkwork:sizeBytes', value: String(bytes.length) }] }],
  };
  const provenance = {
    _type: 'https://in-toto.io/Statement/v1',
    subject: [{ name: paths.artifactRelativePath, digest: { sha256: digest } }],
    predicateType: 'https://slsa.dev/provenance/v1',
    predicate: {
      buildDefinition: {
        buildType: 'https://sdkwork.com/buildtypes/github-workflow/v1',
        externalParameters: { packageId: paths.packageId, runtimeTarget: required(env.SDKWORK_RUNTIME_TARGET, 'SDKWORK_RUNTIME_TARGET') },
        internalParameters: { sourceCommit },
        resolvedDependencies: [{ uri: 'git+https://github.com/Sdkwork-Cloud/sdkwork-aiot', digest: { gitCommit: sourceCommit } }],
      },
      runDetails: { builder: { id: 'https://github.com/Sdkwork-Cloud/sdkwork-github-workflow' }, metadata: { invocationId: String(env.GITHUB_RUN_ID ?? 'local-validation') } },
    },
  };
  fs.writeFileSync(paths.sbomPath, `${JSON.stringify(sbom, null, 2)}\n`);
  fs.writeFileSync(paths.provenancePath, `${JSON.stringify(provenance)}\n`);
  evidenceWriter({ env, paths, root, sourceCommit });
  return { ...paths, digest: `sha256:${digest}` };
}

function writeWorkflowEvidence({ env, paths, root, sourceCommit }) {
  const cli = required(env.SDKWORK_WORKFLOW_CLI, 'SDKWORK_WORKFLOW_CLI');
  const outputs = required(env.SDKWORK_ARTIFACT_EVIDENCE_PATHS, 'SDKWORK_ARTIFACT_EVIDENCE_PATHS').split(/\r?\n/u).filter(Boolean);
  for (const output of outputs) {
    const args = [cli, 'evidence:create', '--config', 'sdkwork.workflow.json', '--target-id', required(env.SDKWORK_PACKAGE_TARGET_ID, 'SDKWORK_PACKAGE_TARGET_ID'), '--deployment-profile', required(env.SDKWORK_DEPLOYMENT_PROFILE, 'SDKWORK_DEPLOYMENT_PROFILE'), '--version', required(env.SDKWORK_PACKAGE_VERSION, 'SDKWORK_PACKAGE_VERSION'), '--source-commit', sourceCommit, '--artifact-id', paths.packageId, '--artifact', paths.artifactRelativePath, '--artifact-evidence', output, '--sbom', portable(path.relative(root, paths.sbomPath)), '--provenance', portable(path.relative(root, paths.provenancePath)), '--signature', portable(path.relative(root, paths.signaturePath))];
    const result = spawnSync(process.execPath, args, { cwd: root, env, stdio: 'inherit' });
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`artifact evidence creation failed with exit code ${result.status ?? 1}`);
  }
}

function gitHead(root) { return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(); }
function sha256(value) { return createHash('sha256').update(value).digest('hex'); }
function portable(value) { return value.split(path.sep).join('/'); }

async function main([command] = process.argv.slice(2)) {
  if (command === 'sign') signReleaseArtifact();
  else if (command === 'attest') createSbomAndProvenance();
  else throw new Error('command must be sign or attest');
}

if (process.argv[1] && path.resolve(process.argv[1]) === MODULE_PATH) {
  main().catch((error) => { console.error(`[sdkwork-aiot-supply-chain] ${error.message}`); process.exitCode = 1; });
}
