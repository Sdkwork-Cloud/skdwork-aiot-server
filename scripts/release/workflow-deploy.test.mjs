import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import { createDeployPlan } from './workflow-deploy.mjs';

const root = path.resolve('.runtime', 'tests', 'aiot-workflow-deploy');

test.afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

test('creates an explicit deployctl apply plan from immutable evidence', () => {
  const evidence = path.join(root, 'evidence.json');
  fs.mkdirSync(root, { recursive: true });
  fs.writeFileSync(evidence, JSON.stringify({ artifactId: 'aiot-0.1.0', digest: `sha256:${'b'.repeat(64)}` }));
  const plan = createDeployPlan({
    root,
    env: {
      SDKWORK_DEPLOYMENT_PROFILE: 'standalone',
      SDKWORK_DEPLOY_ENVIRONMENT: 'production',
      SDKWORK_ARTIFACT_EVIDENCE_PATH: evidence,
      SDKWORK_DEPLOY_ROLLBACK_TARGET: 'release-0.0.9',
      SDKWORK_DEPLOY_APPROVAL_REF: 'change-1234',
    },
  });
  assert.ok(plan.args.includes('standalone.production'));
  assert.ok(plan.args.includes('release-0.0.9'));
  assert.ok(plan.args.includes('change-1234'));
});

test('rejects side-effect selection without explicit approval', () => {
  const evidence = path.join(root, 'evidence.json');
  fs.mkdirSync(root, { recursive: true });
  fs.writeFileSync(evidence, JSON.stringify({ artifactId: 'aiot', digest: `sha256:${'c'.repeat(64)}` }));
  assert.throws(() => createDeployPlan({
    root,
    env: {
      SDKWORK_DEPLOYMENT_PROFILE: 'cloud',
      SDKWORK_DEPLOY_ENVIRONMENT: 'production',
      SDKWORK_ARTIFACT_EVIDENCE_PATH: evidence,
      SDKWORK_DEPLOY_ROLLBACK_TARGET: 'release-0.0.9',
    },
  }), /SDKWORK_DEPLOY_APPROVAL_REF/u);
});
