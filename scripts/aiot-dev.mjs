#!/usr/bin/env node

import { spawn, spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import {
  DEFAULT_DEV_PROFILE_ID,
  listHealthSurfaces,
  listOrchestrationProcesses,
  loadProfile,
  mergeRuntimeEnv,
  REPO_ROOT,
  resolveDevProfileFromDeploymentProfile,
  resolveGatewayBind,
  resolveSurfaceHttpUrl,
  shouldAutostartGateway,
  waitForHttpHealthy,
} from './lib/aiot-topology.mjs';
import {
  assertSupportedDevDatabaseEngine,
  mergeDeviceDatabaseEnv,
  mergeProcessRuntimeEnv,
} from './lib/aiot-device-database.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const API_GATEWAY_REPO = path.resolve(REPO_ROOT, '..', 'sdkwork-api-cloud-gateway');
const HEALTH_PATH = '/healthz';
const HEALTH_TIMEOUT_MS = 2000;
const STARTUP_WAIT_MS = 500;
const MAX_STARTUP_ATTEMPTS = 60;

function cargoCommand() {
  return process.platform === 'win32' ? 'cargo.exe' : 'cargo';
}

function parseArgs(argv) {
  const settings = {
    deploymentProfile: 'standalone',
    serviceLayout: 'split-services',
    database: 'sqlite',
    withSimulator: false,
    dryRun: false,
    help: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--help' || arg === '-h') {
      settings.help = true;
      continue;
    }
    if (arg === '--deployment-profile') {
      settings.deploymentProfile = argv[index + 1] ?? settings.deploymentProfile;
      index += 1;
      continue;
    }
    if (arg === '--service-layout') {
      settings.serviceLayout = argv[index + 1] ?? settings.serviceLayout;
      index += 1;
      continue;
    }
    if (arg === '--database') {
      settings.database = argv[index + 1] ?? settings.database;
      index += 1;
      continue;
    }
    if (arg === '--with-simulator') {
      settings.withSimulator = true;
      continue;
    }
    if (arg === '--hosting') {
      throw new Error(
        '--hosting is retired; use --deployment-profile standalone or --deployment-profile cloud',
      );
    }
    if (arg === '--topology') {
      throw new Error(
        '--topology is retired; use --deployment-profile standalone or --deployment-profile cloud',
      );
    }
    if (arg === '--dry-run') {
      settings.dryRun = true;
    }
  }

  return settings;
}

function printHelp() {
  console.log(`Usage: node scripts/aiot-dev.mjs [options]

Topology-aware AIoT dev entry. Loads configs/topology profile env via @sdkwork/app-topology.

Options:
  --deployment-profile <standalone|cloud>           Default: standalone
  --service-layout <split-services>                 Default: split-services
  --database <sqlite|postgres>                      Default: sqlite
  --with-simulator                                  Also start sdkwork-aiot-xiaozhi-simulator-ui
  --dry-run                                         Print plan without executing
  --help, -h
`);
}

function spawnProcessEntry(entry) {
  const child = spawn(entry.command, entry.args, {
    cwd: entry.cwd ?? REPO_ROOT,
    env: entry.env,
    stdio: 'inherit',
    shell: false,
    windowsHide: true,
  });
  return child;
}

function terminateProcessTree(child) {
  if (!child?.pid) {
    return;
  }
  if (process.platform === 'win32') {
    spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {
      stdio: 'ignore',
      windowsHide: true,
    });
    return;
  }
  child.kill();
}

function createCargoServiceProcess({ label, packageName, binary, env }) {
  const args = ['run', '-p', packageName];
  if (binary && binary !== packageName) {
    args.push('--bin', binary);
  }
  return {
    label,
    command: cargoCommand(),
    args,
    cwd: REPO_ROOT,
    env,
  };
}

function createPlatformGatewayProcess(env) {
  const bind = resolveGatewayBind(env, env.SDKWORK_AIOT_HOSTING ?? 'self-hosted');
  return {
    label: 'sdkwork-api-cloud-gateway',
    command: cargoCommand(),
    args: [
      'run',
      '-p',
      'sdkwork-api-cloud-gateway-api-server',
      '--bin',
      'sdkwork-api-cloud-gateway',
    ],
    cwd: API_GATEWAY_REPO,
    env: {
      ...env,
      SDKWORK_API_CLOUD_GATEWAY_BIND: bind,
    },
  };
}

function shouldSpawnPlatformGateway(processSpec, env) {
  return processSpec.required === true || shouldAutostartGateway(env);
}

function buildProcessEntries(profileId, env, { withSimulator = false } = {}) {
  const entries = [];
  for (const processSpec of listOrchestrationProcesses(profileId)) {
    if (processSpec.id === 'platform.api-gateway') {
      if (!shouldSpawnPlatformGateway(processSpec, env)) {
        continue;
      }
      entries.push(createPlatformGatewayProcess(env));
      continue;
    }
    if (processSpec.crate) {
      entries.push(
        createCargoServiceProcess({
          label: processSpec.id,
          packageName: processSpec.crate,
          binary: processSpec.binary,
          env: mergeProcessRuntimeEnv(processSpec, env),
        }),
      );
    }
  }

  if (withSimulator) {
    entries.push(
      createCargoServiceProcess({
        label: 'sdkwork-aiot-xiaozhi-simulator-ui',
        packageName: 'sdkwork-aiot-xiaozhi-simulator-ui',
        binary: 'sdkwork-aiot-xiaozhi-simulator-ui',
        env,
      }),
    );
  }

  return entries;
}

async function waitForSurfaceHealth(profileId, env) {
  for (const surfaceId of listHealthSurfaces(profileId)) {
    const url = resolveSurfaceHttpUrl(env, surfaceId);
    if (!url) {
      continue;
    }
    let ready = false;
    for (let attempt = 0; attempt < MAX_STARTUP_ATTEMPTS; attempt += 1) {
      ready = await waitForHttpHealthy(url, {
        path: HEALTH_PATH,
        timeoutMs: HEALTH_TIMEOUT_MS,
        attempts: 1,
        intervalMs: STARTUP_WAIT_MS,
      });
      if (ready) {
        console.log(`[sdkwork-aiot] healthy ${surfaceId} (${url}${HEALTH_PATH})`);
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, STARTUP_WAIT_MS));
    }
    if (!ready) {
      throw new Error(`timed out waiting for ${surfaceId} health at ${url}${HEALTH_PATH}`);
    }
  }
}

async function main() {
  const settings = parseArgs(process.argv.slice(2));
  if (settings.help) {
    printHelp();
    process.exit(0);
  }

  assertSupportedDevDatabaseEngine(settings.database);

  const profileId = resolveDevProfileFromDeploymentProfile(
    settings.deploymentProfile,
    settings.serviceLayout,
  ) || DEFAULT_DEV_PROFILE_ID;
  const profileEnv = loadProfile(profileId);
  const runtimeEnv = mergeDeviceDatabaseEnv(
    mergeRuntimeEnv(process.env, profileEnv, {
      SDKWORK_AIOT_PROFILE_ID: profileId,
      SDKWORK_AIOT_DEV_MODE: '1',
    }),
    { databaseEngine: settings.database },
  );

  const processes = buildProcessEntries(profileId, runtimeEnv, {
    withSimulator: settings.withSimulator,
  });

  if (settings.dryRun) {
    console.log(`[sdkwork-aiot] profile=${profileId}`);
    for (const entry of processes) {
      console.log(`[${entry.label}] ${entry.command} ${entry.args.join(' ')}`);
    }
    process.exit(0);
  }

  const children = [];
  let shuttingDown = false;

  function shutdown(exceptChild) {
    if (shuttingDown) {
      return;
    }
    shuttingDown = true;
    for (const child of children) {
      if (child !== exceptChild && child.exitCode == null && child.signalCode == null) {
        terminateProcessTree(child);
      }
    }
  }

  function attachProcessLifecycle(entry, child) {
    child.on('error', (error) => {
      process.stderr.write(
        `[${entry.label}] ${error instanceof Error ? error.message : String(error)}\n`,
      );
      shutdown(child);
      process.exitCode = 1;
    });
    child.on('exit', (code, signal) => {
      if (shuttingDown) {
        return;
      }
      shutdown(child);
      if (code && code !== 0) {
        process.stderr.write(`[${entry.label}] exited with code ${code}\n`);
        process.exitCode = code;
        return;
      }
      if (signal) {
        process.stderr.write(`[${entry.label}] exited with signal ${signal}\n`);
        process.exitCode = 1;
      }
    });
  }

  for (const entry of processes) {
    const child = spawnProcessEntry(entry);
    children.push(child);
    attachProcessLifecycle(entry, child);
  }

  try {
    await waitForSurfaceHealth(profileId, runtimeEnv);
  } catch (error) {
    shutdown();
    throw error;
  }

  console.log(`[sdkwork-aiot] dev stack ready (profile=${profileId})`);
  const stop = () => shutdown();
  process.once('SIGINT', stop);
  process.once('SIGTERM', stop);
}

main().catch((error) => {
  console.error(`[sdkwork-aiot] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});
