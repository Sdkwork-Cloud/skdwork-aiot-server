import {
  createClient,
  type SdkworkAppClient,
  type SdkworkAppConfig,
} from '@sdkwork/voice-app-sdk';
import { readOptionalBearerToken } from '@sdkwork/aiot-app-core';

import { getAiotH5TokenManager } from './h5TokenManager';
import { readH5RuntimeSession } from './h5RuntimeSession';
import { isVoiceAppSdkConfigured, resolveVoiceAppApiBaseUrl } from './siblingAppUrls';

export type SdkworkVoiceAppClient = SdkworkAppClient;
export type SdkworkVoiceAppClientConfig = SdkworkAppConfig;

let voiceAppSdkClient: SdkworkVoiceAppClient | null = null;

export { isVoiceAppSdkConfigured };

export function createVoiceAppSdkClientConfig(): SdkworkVoiceAppClientConfig {
  const session = readH5RuntimeSession();
  return {
    baseUrl: resolveVoiceAppApiBaseUrl(),
    authToken: readOptionalBearerToken(session.authToken),
    accessToken: readOptionalBearerToken(session.accessToken),
    platform: 'h5',
  };
}

export function initVoiceAppSdkClient(
  config: SdkworkVoiceAppClientConfig = createVoiceAppSdkClientConfig(),
): SdkworkVoiceAppClient {
  const client = createClient(config);
  client.setTokenManager(getAiotH5TokenManager());
  voiceAppSdkClient = client;
  return voiceAppSdkClient;
}

export function getVoiceAppSdkClient(): SdkworkVoiceAppClient {
  return voiceAppSdkClient ?? initVoiceAppSdkClient();
}

export function resetVoiceAppSdkClient(): void {
  voiceAppSdkClient = null;
}
