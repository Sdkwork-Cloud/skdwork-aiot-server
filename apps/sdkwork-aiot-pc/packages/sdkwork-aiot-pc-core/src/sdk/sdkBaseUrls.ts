import {
  readFirstNonBlank,
  readImportMetaEnv,
  readProcessEnv,
} from '@sdkwork/aiot-app-core';
import {
  DEFAULT_LOCAL_APPLICATION_ADMIN_HTTP_URL,
  DEFAULT_LOCAL_APPLICATION_APP_HTTP_URL,
  DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_HTTP_URL,
  DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_WEBSOCKET_URL,
  DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL,
  DEFAULT_LOCAL_AGENTS_APP_HTTP_URL,
  DEFAULT_LOCAL_VOICE_APP_HTTP_URL,
  VITE_SDKWORK_AIOT_APPLICATION_ADMIN_HTTP_URL,
  VITE_SDKWORK_AIOT_APPLICATION_APP_HTTP_URL,
  VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL,
  VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_WEBSOCKET_URL,
  VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL,
  VITE_SDKWORK_DRIVE_APP_API_BASE_URL,
  VITE_SDKWORK_AGENTS_APP_API_BASE_URL,
  VITE_SDKWORK_VOICE_APP_API_BASE_URL,
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

function resolveLocalDevApplicationAppHttpBaseUrl(): string | undefined {
  return isSdkRuntimeDev() ? DEFAULT_LOCAL_APPLICATION_APP_HTTP_URL : undefined;
}

function resolveLocalDevApplicationAdminHttpBaseUrl(): string | undefined {
  return isSdkRuntimeDev() ? DEFAULT_LOCAL_APPLICATION_ADMIN_HTTP_URL : undefined;
}

function resolveLocalDevEdgeIngressHttpBaseUrl(): string | undefined {
  return isSdkRuntimeDev() ? DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_HTTP_URL : undefined;
}

function resolveLocalDevEdgeIngressWebSocketBaseUrl(): string | undefined {
  return isSdkRuntimeDev() ? DEFAULT_LOCAL_EDGE_DEVICE_INGRESS_WEBSOCKET_URL : undefined;
}

function resolveLocalDevPlatformApiGatewayBaseUrl(): string | undefined {
  return isSdkRuntimeDev() ? DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL : undefined;
}

export function resolveAiotAppApiBaseUrl(): string {
  const value = readFirstNonBlank([
    readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_APPLICATION_APP_HTTP_URL),
    resolveLocalDevApplicationAppHttpBaseUrl(),
    resolveSameOriginHttpBaseUrl(),
    DEFAULT_LOCAL_APPLICATION_APP_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_APPLICATION_APP_HTTP_URL;
  return normalizeHttpSdkBaseUrl(value);
}

export function resolveAiotAdminApiBaseUrl(): string {
  const value = readFirstNonBlank([
    readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_APPLICATION_ADMIN_HTTP_URL),
    resolveLocalDevApplicationAdminHttpBaseUrl(),
    resolveSameOriginHttpBaseUrl(),
    DEFAULT_LOCAL_APPLICATION_ADMIN_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_APPLICATION_ADMIN_HTTP_URL;
  return normalizeHttpSdkBaseUrl(value);
}

export function resolveAiotPlatformApiGatewayBaseUrl(): string {
  const value = readFirstNonBlank([
    readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL),
    resolveLocalDevPlatformApiGatewayBaseUrl(),
    resolveSameOriginHttpBaseUrl(),
    DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL;
  return normalizeHttpSdkBaseUrl(value);
}

export function resolveDriveAppApiBaseUrl(): string {
  const value = readFirstNonBlank([
    readSdkBaseUrlEnvValue(VITE_SDKWORK_DRIVE_APP_API_BASE_URL),
    readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL),
    resolveLocalDevPlatformApiGatewayBaseUrl(),
    resolveSameOriginHttpBaseUrl(),
    DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL;
  return normalizeHttpSdkBaseUrl(value);
}

export function isAgentsAppSdkConfigured(): boolean {
  if (readSdkBaseUrlEnvValue(VITE_SDKWORK_AGENTS_APP_API_BASE_URL)) {
    return true;
  }
  return isSdkRuntimeDev()
    && Boolean(readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL));
}

export function resolveAgentsAppApiBaseUrl(): string {
  const value = readFirstNonBlank([
    readSdkBaseUrlEnvValue(VITE_SDKWORK_AGENTS_APP_API_BASE_URL),
    readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL),
    resolveLocalDevPlatformApiGatewayBaseUrl(),
    isSdkRuntimeDev() ? DEFAULT_LOCAL_AGENTS_APP_HTTP_URL : undefined,
    resolveSameOriginHttpBaseUrl(),
    DEFAULT_LOCAL_AGENTS_APP_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_AGENTS_APP_HTTP_URL;
  return normalizeHttpSdkBaseUrl(value);
}

export function isVoiceAppSdkConfigured(): boolean {
  if (readSdkBaseUrlEnvValue(VITE_SDKWORK_VOICE_APP_API_BASE_URL)) {
    return true;
  }
  return isSdkRuntimeDev()
    && Boolean(readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL));
}

export function resolveVoiceAppApiBaseUrl(): string {
  const value = readFirstNonBlank([
    readSdkBaseUrlEnvValue(VITE_SDKWORK_VOICE_APP_API_BASE_URL),
    readSdkBaseUrlEnvValue(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL),
    resolveLocalDevPlatformApiGatewayBaseUrl(),
    isSdkRuntimeDev() ? DEFAULT_LOCAL_VOICE_APP_HTTP_URL : undefined,
    resolveSameOriginHttpBaseUrl(),
    DEFAULT_LOCAL_VOICE_APP_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_VOICE_APP_HTTP_URL;
  return normalizeHttpSdkBaseUrl(value);
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
