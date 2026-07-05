import { readPcReactRuntimeSession } from '@sdkwork/core-pc-react';
import {
  createClient,
  type SdkworkAppClient,
  type SdkworkAppConfig,
} from '@sdkwork/voice-app-sdk';
import { readOptionalBearerToken } from '@sdkwork/aiot-app-core';

import { getAiotPcTokenManager } from './pcTokenManager';
import { isVoiceAppSdkConfigured, resolveVoiceAppApiBaseUrl } from './sdkBaseUrls';

export type SdkworkVoiceAppClient = SdkworkAppClient;
export type SdkworkVoiceAppClientConfig = SdkworkAppConfig;

let voiceAppSdkClient: SdkworkVoiceAppClient | null = null;

export { isVoiceAppSdkConfigured };

export function createVoiceAppSdkClientConfig(
  session = readPcReactRuntimeSession(),
): SdkworkVoiceAppClientConfig {
  return {
    baseUrl: resolveVoiceAppApiBaseUrl(),
    authToken: readOptionalBearerToken(session.authToken),
    accessToken: readOptionalBearerToken(session.accessToken),
    platform: 'pc',
  };
}

export function initVoiceAppSdkClient(
  config: SdkworkVoiceAppClientConfig = createVoiceAppSdkClientConfig(),
): SdkworkVoiceAppClient {
  const client = createClient(config);
  client.setTokenManager(getAiotPcTokenManager());
  voiceAppSdkClient = client;
  return voiceAppSdkClient;
}

export function getVoiceAppSdkClient(): SdkworkVoiceAppClient {
  return voiceAppSdkClient ?? initVoiceAppSdkClient();
}

export function resetVoiceAppSdkClient(): void {
  voiceAppSdkClient = null;
}
