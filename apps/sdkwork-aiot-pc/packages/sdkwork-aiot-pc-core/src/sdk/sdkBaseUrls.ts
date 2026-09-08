import {
  readFirstNonBlank,
  readImportMetaEnv,
  readProcessEnv,
} from '@sdkwork/aiot-app-core';
import { resolveBaseUrl } from '@sdkwork/sdk-common';
import {
  DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_HTTP_URL,
  DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_WEBSOCKET_URL,
  VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL,
  VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_WEBSOCKET_URL,
} from './topologyEnvKeys';

const SDKWORK_APP_API_PREFIX = '/app/v3/api';
const SDKWORK_IOT_APP_API_PREFIX = '/app/v3/api/iot';
const SDKWORK_BACKEND_API_PREFIX = '/backend/v3/api';
const SDKWORK_IOT_BACKEND_API_PREFIX = '/backend/v3/api/iot';

type RuntimeImportMetaEnv = Record<string, string | boolean | undefined> & {
  DEV?: boolean | 'true' | 'false';
};

function readRuntimeImportMetaEnv(): RuntimeImportMetaEnv {
  return (import.meta.env ?? {}) as RuntimeImportMetaEnv;
}

export function readSdkBaseUrlEnvValue(key: string): string | undefined {
  return readImportMetaEnv(key);
}

function readNodeEnvValue(key: string): string | undefined {
  return readProcessEnv(key);
}

export function isSdkRuntimeDev(): boolean {
  const env = readRuntimeImportMetaEnv();
  if (env.DEV === true || env.DEV === 'true') {
    return true;
  }
  if (env.DEV === false || env.DEV === 'false') {
    return false;
  }
  const nodeEnv = readNodeEnvValue('NODE_ENV');
  if (nodeEnv) {
    return nodeEnv !== 'production';
  }
  return typeof window === 'undefined';
}

function stripSdkOwnedPathSuffix(pathname: string, suffixes: string[]): string {
  const normalizedPathname = pathname.replace(/\/+$/u, '');
  if (!normalizedPathname || normalizedPathname === '/') {
    return '';
  }

  for (const suffix of suffixes) {
    const normalizedSuffix = `/${suffix.replace(/^\/+|\/+$/gu, '')}`;
    if (normalizedPathname === normalizedSuffix) {
      return '';
    }
    if (normalizedPathname.endsWith(normalizedSuffix)) {
      return normalizedPathname.slice(0, -normalizedSuffix.length) || '';
    }
  }

  return normalizedPathname;
}

export function normalizeHttpSdkBaseUrl(
  value: string,
  sdkOwnedPathSuffixes: string[] = [
    SDKWORK_APP_API_PREFIX,
    SDKWORK_IOT_APP_API_PREFIX,
    SDKWORK_BACKEND_API_PREFIX,
    SDKWORK_IOT_BACKEND_API_PREFIX,
  ],
): string {
  try {
    const parsedUrl = new URL(value);
    if (parsedUrl.protocol !== 'http:' && parsedUrl.protocol !== 'https:') {
      return value;
    }
    const normalizedPathname = stripSdkOwnedPathSuffix(parsedUrl.pathname, sdkOwnedPathSuffixes);
    return `${parsedUrl.origin}${normalizedPathname}`;
  } catch {
    return value;
  }
}

export function normalizeWebSocketSdkBaseUrl(value: string): string {
  try {
    const parsedUrl = new URL(value);
    if (parsedUrl.protocol !== 'ws:' && parsedUrl.protocol !== 'wss:') {
      return value;
    }
    return `${parsedUrl.origin}`;
  } catch {
    return value;
  }
}

function resolveSameOriginHttpBaseUrl(): string | undefined {
  if (typeof window === 'undefined') {
    return undefined;
  }
  return window.location.origin;
}

function resolveLocalDevEdgeIngressHttpBaseUrl(): string | undefined {
  return isSdkRuntimeDev() ? DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_HTTP_URL : undefined;
}

function resolveLocalDevEdgeIngressWebSocketBaseUrl(): string | undefined {
  return isSdkRuntimeDev() ? DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_WEBSOCKET_URL : undefined;
}

function resolveSdkApiBaseUrl(): string {
  // Unified single base-url key (SDKWORK_API_BASE_URL); the matching API host
  // is chosen from the current page's environment+brand. All SDK clients in
  // this package expect a bare origin — SDK-owned path prefixes (/app/v3/api
  // etc.) are appended by the generated SDK clients themselves, so path
  // preservation stays off.
  return resolveBaseUrl({ envKey: 'SDKWORK_API_BASE_URL' }).url;
}

export function resolveAiotAppApiBaseUrl(): string {
  return resolveSdkApiBaseUrl();
}

export function resolveAiotAdminApiBaseUrl(): string {
  return resolveSdkApiBaseUrl();
}

export function resolveAiotPlatformApiGatewayBaseUrl(): string {
  return resolveSdkApiBaseUrl();
}

export function resolveDriveAppApiBaseUrl(): string {
  return resolveSdkApiBaseUrl();
}

export function isAgentsAppSdkConfigured(): boolean {
  // The unified single base-url key drives sibling-app SDK availability.
  return resolveBaseUrl({ envKey: 'SDKWORK_API_BASE_URL' }).reason !== 'empty';
}

export function resolveAgentsAppApiBaseUrl(): string {
  return resolveSdkApiBaseUrl();
}

export function isVoiceAppSdkConfigured(): boolean {
  return resolveBaseUrl({ envKey: 'SDKWORK_API_BASE_URL' }).reason !== 'empty';
}

export function resolveVoiceAppApiBaseUrl(): string {
  return resolveSdkApiBaseUrl();
}

export function resolveAiotEdgeIngressHttpBaseUrl(): string {
  const value = readFirstNonBlank([
    readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL),
    resolveLocalDevEdgeIngressHttpBaseUrl(),
    resolveSameOriginHttpBaseUrl(),
    DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_HTTP_URL;
  return normalizeHttpSdkBaseUrl(value);
}

export function resolveAiotEdgeIngressWebSocketBaseUrl(): string {
  const explicit = readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_WEBSOCKET_URL);
  if (explicit) {
    return normalizeWebSocketSdkBaseUrl(explicit);
  }
  const httpBaseUrl = resolveAiotEdgeIngressHttpBaseUrl();
  try {
    const parsedUrl = new URL(normalizeHttpSdkBaseUrl(httpBaseUrl));
    parsedUrl.protocol = parsedUrl.protocol === 'https:' ? 'wss:' : 'ws:';
    return normalizeWebSocketSdkBaseUrl(parsedUrl.toString());
  } catch {
    return (
      resolveLocalDevEdgeIngressWebSocketBaseUrl()
      ?? DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_WEBSOCKET_URL
    );
  }
}
