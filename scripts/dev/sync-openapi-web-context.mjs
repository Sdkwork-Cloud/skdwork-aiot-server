#!/usr/bin/env node
/**
 * Materializes SDKWork OpenAPI contract extensions required by WEB_FRAMEWORK_SPEC.md
 * and API_SPEC.md (ownership, request context, api surface, list pagination).
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..', '..');

const authorities = [
  {
    relativePath: 'apis/app-api/iot/sdkwork-aiot-app-api.openapi.json',
    apiSurface: 'app-api',
    owner: 'sdkwork-aiot',
    apiAuthority: 'sdkwork-aiot-app-api',
  },
  {
    relativePath: 'apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json',
    apiSurface: 'backend-api',
    owner: 'sdkwork-aiot',
    apiAuthority: 'sdkwork-aiot-backend-api',
  },
];

/** Offset-mode list parameters only — handlers implement page/page_size per PAGINATION_SPEC.md. */
const LIST_QUERY_PARAMETERS = [
  {
    name: 'page',
    in: 'query',
    description: 'One-based page index.',
    schema: { type: 'integer', minimum: 1, default: 1 },
  },
  {
    name: 'page_size',
    in: 'query',
    description: 'Page size (max 200).',
    schema: { type: 'integer', minimum: 1, maximum: 200, default: 20 },
  },
];

const LEGACY_UNIMPLEMENTED_LIST_PARAMETERS = new Set(['cursor', 'sort', 'q']);

const PAGE_INFO_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['mode', 'page', 'pageSize', 'total', 'hasMore'],
  properties: {
    mode: {
      type: 'string',
      enum: ['offset', 'cursor'],
      description: 'Pagination mode for this collection.',
    },
    page: { type: 'integer', minimum: 1 },
    pageSize: { type: 'integer', minimum: 1, maximum: 200 },
    total: { type: 'integer', minimum: 0 },
    hasMore: { type: 'boolean' },
  },
};

function mergeListParameters(existing = []) {
  const filtered = existing.filter(
    (param) => !LEGACY_UNIMPLEMENTED_LIST_PARAMETERS.has(param.name),
  );
  const names = new Set(filtered.map((param) => param.name));
  const merged = [...filtered];
  for (const param of LIST_QUERY_PARAMETERS) {
    if (!names.has(param.name)) {
      merged.push(param);
    }
  }
  return merged;
}

function ensurePageInfoOnListSchemas(openapi) {
  openapi.components ??= {};
  openapi.components.schemas ??= {};
  openapi.components.schemas.PageInfo = PAGE_INFO_SCHEMA;

  for (const [name, schema] of Object.entries(openapi.components.schemas)) {
    if (!name.endsWith('ListResponse') && name !== 'StandardCollectionResponse') {
      continue;
    }
    if (!schema.properties?.data) {
      continue;
    }
    schema.properties.pageInfo = { $ref: '#/components/schemas/PageInfo' };
    if (!schema.required?.includes('pageInfo')) {
      schema.required = [...(schema.required ?? []), 'pageInfo'];
    }
  }
}

function normalizeSdkworkV3SecuritySchemes(openapi) {
  openapi.components ??= {};
  openapi.components.securitySchemes = {
    AuthToken: {
      type: 'http',
      scheme: 'bearer',
      bearerFormat: 'JWT',
      description: 'Authorization: Bearer <auth_token>',
    },
    AccessToken: {
      type: 'apiKey',
      in: 'header',
      name: 'Access-Token',
      description: 'Signed JWT access_token compact serialization (header.payload.signature).',
    },
  };

  for (const pathItem of Object.values(openapi.paths ?? {})) {
    for (const operation of Object.values(pathItem ?? {})) {
      if (!Array.isArray(operation?.security)) {
        continue;
      }
      operation.security = operation.security.map((requirement) => {
        const next = { ...requirement };
        if (Object.prototype.hasOwnProperty.call(next, 'Authorization')) {
          next.AuthToken = next.Authorization;
          delete next.Authorization;
        }
        if (Object.prototype.hasOwnProperty.call(next, 'Access-Token')) {
          next.AccessToken = next['Access-Token'];
          delete next['Access-Token'];
        }
        if (Object.prototype.hasOwnProperty.call(next, 'SdkworkAccessToken')) {
          next.AccessToken = next.SdkworkAccessToken;
          delete next.SdkworkAccessToken;
        }
        return next;
      });
    }
  }
}

let changed = 0;
const checkOnly = process.argv.includes('--check');

for (const authority of authorities) {
  const absolutePath = path.join(repoRoot, authority.relativePath);
  const original = fs.readFileSync(absolutePath, 'utf8');
  const openapi = JSON.parse(original.replace(/^\uFEFF/u, ''));

  for (const pathItem of Object.values(openapi.paths ?? {})) {
    for (const operation of Object.values(pathItem ?? {})) {
      if (!operation || typeof operation !== 'object' || !operation.operationId) {
        continue;
      }

      if (operation['x-sdkwork-request-context'] !== 'WebRequestContext') {
        operation['x-sdkwork-request-context'] = 'WebRequestContext';
        changed += 1;
      }
      if (operation['x-sdkwork-api-surface'] !== authority.apiSurface) {
        operation['x-sdkwork-api-surface'] = authority.apiSurface;
        changed += 1;
      }
      if (operation['x-sdkwork-owner'] !== authority.owner) {
        operation['x-sdkwork-owner'] = authority.owner;
        changed += 1;
      }
      if (operation['x-sdkwork-api-authority'] !== authority.apiAuthority) {
        operation['x-sdkwork-api-authority'] = authority.apiAuthority;
        changed += 1;
      }

      if (operation.operationId.endsWith('.list')) {
        const nextParameters = mergeListParameters(operation.parameters ?? []);
        if (JSON.stringify(nextParameters) !== JSON.stringify(operation.parameters ?? [])) {
          operation.parameters = nextParameters;
          changed += 1;
        }
      }
    }
  }

  ensurePageInfoOnListSchemas(openapi);
  normalizeSdkworkV3SecuritySchemes(openapi);

  const next = `${JSON.stringify(openapi, null, 2)}\n`;
  if (checkOnly) {
    if (next !== original.replace(/^\uFEFF/u, '')) {
      console.error(`OpenAPI materialization drift detected in ${authority.relativePath}`);
      process.exit(1);
    }
    continue;
  }

  fs.writeFileSync(absolutePath, next, 'utf8');
}

if (checkOnly) {
  console.log('OpenAPI WebRequestContext extensions are materialized');
  process.exit(0);
}

console.log(`OpenAPI contract extensions updated (${changed} field writes)`);
