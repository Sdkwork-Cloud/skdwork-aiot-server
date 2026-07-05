import { readPcReactRuntimeSession } from '@sdkwork/core-pc-react';
import {
  createClient,
  sendAgentChatMessageSync,
  type SdkworkAppClient,
  type SdkworkAppConfig,
} from '@sdkwork/agents-app-sdk';
import { readOptionalBearerToken } from '@sdkwork/aiot-app-core';

import { getAiotPcTokenManager } from './pcTokenManager';
import { isAgentsAppSdkConfigured, resolveAgentsAppApiBaseUrl } from './sdkBaseUrls';

export type SdkworkAgentsAppClient = SdkworkAppClient;
export type SdkworkAgentsAppClientConfig = SdkworkAppConfig;

let agentsAppSdkClient: SdkworkAgentsAppClient | null = null;

export { isAgentsAppSdkConfigured, sendAgentChatMessageSync };

export function createAgentsAppSdkClientConfig(
  session = readPcReactRuntimeSession(),
): SdkworkAgentsAppClientConfig {
  return {
    baseUrl: resolveAgentsAppApiBaseUrl(),
    authToken: readOptionalBearerToken(session.authToken),
    accessToken: readOptionalBearerToken(session.accessToken),
    platform: 'pc',
  };
}

export function initAgentsAppSdkClient(
  config: SdkworkAgentsAppClientConfig = createAgentsAppSdkClientConfig(),
): SdkworkAgentsAppClient {
  const client = createClient(config);
  client.setTokenManager(getAiotPcTokenManager());
  agentsAppSdkClient = client;
  return agentsAppSdkClient;
}

export function getAgentsAppSdkClient(): SdkworkAgentsAppClient {
  return agentsAppSdkClient ?? initAgentsAppSdkClient();
}

export function resetAgentsAppSdkClient(): void {
  agentsAppSdkClient = null;
}
