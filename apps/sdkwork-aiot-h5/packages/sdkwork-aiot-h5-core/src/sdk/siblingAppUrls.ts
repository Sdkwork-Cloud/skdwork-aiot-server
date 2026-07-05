import {
  readFirstNonBlank,
  readImportMetaEnv,
} from '@sdkwork/aiot-app-core';

export const VITE_SDKWORK_AGENTS_APP_API_BASE_URL = 'VITE_SDKWORK_AGENTS_APP_API_BASE_URL';
export const VITE_SDKWORK_VOICE_APP_API_BASE_URL = 'VITE_SDKWORK_VOICE_APP_API_BASE_URL';
export const VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL =
  'VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL';
export const VITE_SDKWORK_AIOT_AGENTS_DEFAULT_AGENT_ID = 'VITE_SDKWORK_AIOT_AGENTS_DEFAULT_AGENT_ID';
export const VITE_SDKWORK_AIOT_VOICE_DEFAULT_MODEL = 'VITE_SDKWORK_AIOT_VOICE_DEFAULT_MODEL';
export const VITE_SDKWORK_AIOT_VOICE_DEFAULT_VOICE = 'VITE_SDKWORK_AIOT_VOICE_DEFAULT_VOICE';
export const VITE_SDKWORK_AIOT_VOICE_TRANSCRIPTION_MODEL = 'VITE_SDKWORK_AIOT_VOICE_TRANSCRIPTION_MODEL';

export const DEFAULT_LOCAL_AGENTS_APP_HTTP_URL = 'http://127.0.0.1:8095';
export const DEFAULT_LOCAL_VOICE_APP_HTTP_URL = 'http://127.0.0.1:18096';
export const DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL = 'http://127.0.0.1:3900';
export const DEFAULT_AIOT_AGENTS_AGENT_ID = 'agent.aiot.assistant';

function isRuntimeDev(): boolean {
  const env = (import.meta.env ?? {}) as Record<string, string | boolean | undefined>;
  return env.DEV === true || env.DEV === 'true' || env.MODE === 'development';
}

function normalizeHttpBaseUrl(value: string): string {
  try {
    const parsed = new URL(value);
    return parsed.origin;
  } catch {
    return value;
  }
}

export function isAgentsAppSdkConfigured(): boolean {
  return Boolean(
    readImportMetaEnv(VITE_SDKWORK_AGENTS_APP_API_BASE_URL)
    || readImportMetaEnv(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL)
    || isRuntimeDev(),
  );
}

export function resolveAgentsAppApiBaseUrl(): string {
  const value = readFirstNonBlank([
    readImportMetaEnv(VITE_SDKWORK_AGENTS_APP_API_BASE_URL),
    readImportMetaEnv(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL),
    isRuntimeDev() ? DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL : undefined,
    isRuntimeDev() ? DEFAULT_LOCAL_AGENTS_APP_HTTP_URL : undefined,
    typeof window !== 'undefined' ? window.location.origin : undefined,
    DEFAULT_LOCAL_AGENTS_APP_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_AGENTS_APP_HTTP_URL;
  return normalizeHttpBaseUrl(value);
}

export function isVoiceAppSdkConfigured(): boolean {
  return Boolean(
    readImportMetaEnv(VITE_SDKWORK_VOICE_APP_API_BASE_URL)
    || readImportMetaEnv(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL)
    || isRuntimeDev(),
  );
}

export function resolveVoiceAppApiBaseUrl(): string {
  const value = readFirstNonBlank([
    readImportMetaEnv(VITE_SDKWORK_VOICE_APP_API_BASE_URL),
    readImportMetaEnv(VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL),
    isRuntimeDev() ? DEFAULT_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL : undefined,
    isRuntimeDev() ? DEFAULT_LOCAL_VOICE_APP_HTTP_URL : undefined,
    typeof window !== 'undefined' ? window.location.origin : undefined,
    DEFAULT_LOCAL_VOICE_APP_HTTP_URL,
  ]) ?? DEFAULT_LOCAL_VOICE_APP_HTTP_URL;
  return normalizeHttpBaseUrl(value);
}

export function resolveDefaultAiotAgentId(): string {
  const configured = readImportMetaEnv(VITE_SDKWORK_AIOT_AGENTS_DEFAULT_AGENT_ID);
  if (configured) {
    return configured;
  }
  return DEFAULT_AIOT_AGENTS_AGENT_ID;
}
