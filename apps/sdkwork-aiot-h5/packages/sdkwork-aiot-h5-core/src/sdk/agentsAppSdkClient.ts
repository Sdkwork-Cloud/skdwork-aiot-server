import {
  createClient,
  sendAgentChatMessageSync,
  type SdkworkAppClient,
  type SdkworkAppConfig,
} from '@sdkwork/agents-app-sdk';
import { readOptionalBearerToken } from '@sdkwork/aiot-app-core';

import { getAiotH5TokenManager } from './h5TokenManager';
import { readH5RuntimeSession } from './h5RuntimeSession';
import { isAgentsAppSdkConfigured, resolveAgentsAppApiBaseUrl } from './siblingAppUrls';

export type SdkworkAgentsAppClient = SdkworkAppClient;
export type SdkworkAgentsAppClientConfig = SdkworkAppConfig;

let agentsAppSdkClient: SdkworkAgentsAppClient | null = null;

export { isAgentsAppSdkConfigured, sendAgentChatMessageSync };

export function createAgentsAppSdkClientConfig(): SdkworkAgentsAppClientConfig {
  const session = readH5RuntimeSession();
  return {
    baseUrl: resolveAgentsAppApiBaseUrl(),
    authToken: readOptionalBearerToken(session.authToken),
    accessToken: readOptionalBearerToken(session.accessToken),
    platform: 'h5',
  };
}

export function initAgentsAppSdkClient(
  config: SdkworkAgentsAppClientConfig = createAgentsAppSdkClientConfig(),
): SdkworkAgentsAppClient {
  const client = createClient(config);
  client.setTokenManager(getAiotH5TokenManager());
  agentsAppSdkClient = client;
  return agentsAppSdkClient;
}

export function getAgentsAppSdkClient(): SdkworkAgentsAppClient {
  return agentsAppSdkClient ?? initAgentsAppSdkClient();
}

export function resetAgentsAppSdkClient(): void {
  agentsAppSdkClient = null;
}
