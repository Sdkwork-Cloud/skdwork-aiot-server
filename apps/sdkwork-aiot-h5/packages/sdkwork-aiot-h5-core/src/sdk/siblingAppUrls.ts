import { readImportMetaEnv } from '@sdkwork/aiot-app-core';
import { resolveBaseUrl } from '@sdkwork/sdk-common';

export const VITE_SDKWORK_AIOT_AGENTS_DEFAULT_AGENT_ID = 'VITE_SDKWORK_AIOT_AGENTS_DEFAULT_AGENT_ID';
export const VITE_SDKWORK_AIOT_VOICE_DEFAULT_MODEL = 'VITE_SDKWORK_AIOT_VOICE_DEFAULT_MODEL';
export const VITE_SDKWORK_AIOT_VOICE_DEFAULT_VOICE = 'VITE_SDKWORK_AIOT_VOICE_DEFAULT_VOICE';
export const VITE_SDKWORK_AIOT_VOICE_TRANSCRIPTION_MODEL = 'VITE_SDKWORK_AIOT_VOICE_TRANSCRIPTION_MODEL';

export const DEFAULT_AIOT_AGENTS_AGENT_ID = 'agent.aiot.assistant';

export function isAgentsAppSdkConfigured(): boolean {
  // The unified single base-url key drives sibling-app SDK availability.
  return resolveBaseUrl({ envKey: 'SDKWORK_API_BASE_URL' }).reason !== 'empty';
}

export function resolveAgentsAppApiBaseUrl(): string {
  // Single shared base-url key; the matching API host is chosen from the
  // current page's environment+brand. The sibling agents-app SDK client
  // expects a bare origin, so path preservation stays off.
  return resolveBaseUrl({ envKey: 'SDKWORK_API_BASE_URL' }).url;
}

export function isVoiceAppSdkConfigured(): boolean {
  return resolveBaseUrl({ envKey: 'SDKWORK_API_BASE_URL' }).reason !== 'empty';
}

export function resolveVoiceAppApiBaseUrl(): string {
  // Same shared key; the sibling voice-app SDK client also expects a bare
  // origin.
  return resolveBaseUrl({ envKey: 'SDKWORK_API_BASE_URL' }).url;
}

export function resolveDefaultAiotAgentId(): string {
  const configured = readImportMetaEnv(VITE_SDKWORK_AIOT_AGENTS_DEFAULT_AGENT_ID);
  if (configured) {
    return configured;
  }
  return DEFAULT_AIOT_AGENTS_AGENT_ID;
}
