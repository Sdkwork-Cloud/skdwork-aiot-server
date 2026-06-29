#!/usr/bin/env node
/**
 * Thin entrypoint: security scheme normalization lives in sync-openapi-web-context.mjs.
 */
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const script = path.resolve(path.dirname(fileURLToPath(import.meta.url)), 'sync-openapi-web-context.mjs');
const result = spawnSync(process.execPath, [script], { stdio: 'inherit' });
process.exit(result.status ?? 1);
