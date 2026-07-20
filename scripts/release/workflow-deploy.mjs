#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const MODULE_PATH = fileURLToPath(import.meta.url);
const REPO_ROOT = path.resolve(path.dirname(MODULE_PATH), '..', '..');
const DEPLOYCTL = path.resolve(REPO_ROOT, '..', 'sdkwork-specs', 'tools', 'deployctl.mjs');

function required(env, key) {
  const value = String(env[key] ?? '').trim();
  if (!value) throw new Error(`${key} is required for side-effecting deployment`);
  return value;
}

export function createDeployPlan({ env = process.env, root = REPO_ROOT } = {}) {
  const profile = required(env, 'SDKWORK_DEPLOYMENT_PROFILE');
  const environment = required(env, 'SDKWORK_DEPLOY_ENVIRONMENT');
  if (!/^(standalone|cloud)$/u.test(profile)) throw new Error('deployment profile must be standalone or cloud');
  if (!/^(test|staging|production)$/u.test(environment)) throw new Error('deploy environment must be test, staging, or production');
  const evidencePath = path.resolve(required(env, 'SDKWORK_ARTIFACT_EVIDENCE_PATH'));
  if (!fs.existsSync(evidencePath)) throw new Error(`artifact evidence does not exist: ${evidencePath}`);
  const evidence = JSON.parse(fs.readFileSync(evidencePath, 'utf8'));
  if (!evidence.artifactId || !/^sha256:[a-f0-9]{64}$/u.test(evidence.digest ?? '')) throw new Error('artifact evidence must contain artifactId and immutable sha256 digest');
  return {
    command: process.execPath,
    args: [DEPLOYCTL, 'apply', '--root', root, '--profile', `${profile}.${environment}`, '--environment', environment, '--artifact-id', evidence.artifactId, '--artifact-digest', evidence.digest, '--artifact-evidence', evidencePath, '--artifact-root', path.join(root, '.sdkwork', 'artifacts'), '--rollback-target', required(env, 'SDKWORK_DEPLOY_ROLLBACK_TARGET'), '--approval-ref', required(env, 'SDKWORK_DEPLOY_APPROVAL_REF')],
    cwd: root,
  };
}

async function main() {
  const plan = createDeployPlan();
  const result = spawnSync(plan.command, plan.args, { cwd: plan.cwd, env: process.env, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`deployctl apply failed with exit code ${result.status ?? 1}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === MODULE_PATH) {
  main().catch((error) => { console.error(`[sdkwork-aiot-workflow-deploy] ${error.message}`); process.exitCode = 1; });
}
