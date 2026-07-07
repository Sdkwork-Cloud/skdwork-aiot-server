import { readPcReactRuntimeSession } from '@sdkwork/core-pc-react';
import {
  createAiotBackendClient,
  type SdkworkAiotBackendClient,
  type SdkworkAiotBackendClientConfig,
} from '@sdkwork/aiot-backend-sdk';
import { readOptionalBearerToken } from '@sdkwork/aiot-app-core';

import { getAiotPcTokenManager } from './pcTokenManager';
import { resolveAiotAdminApiBaseUrl } from './sdkBaseUrls';

let aiotBackendSdkClient: SdkworkAiotBackendClient | null = null;

export function createAiotBackendSdkClientConfig(
  session = readPcReactRuntimeSession(),
): SdkworkAiotBackendClientConfig {
  return {
    baseUrl: resolveAiotAdminApiBaseUrl(),
    authToken: readOptionalBearerToken(session.authToken),
    accessToken: readOptionalBearerToken(session.accessToken),
    platform: 'pc',
  };
}

export function initAiotBackendSdkClient(
  config: SdkworkAiotBackendClientConfig = createAiotBackendSdkClientConfig(),
): SdkworkAiotBackendClient {
  const client = createAiotBackendClient(config);
  client.setTokenManager(getAiotPcTokenManager());
  aiotBackendSdkClient = client;
  return aiotBackendSdkClient;
}

export function getAiotBackendSdkClient(): SdkworkAiotBackendClient {
  return aiotBackendSdkClient ?? initAiotBackendSdkClient();
}

export function resetAiotBackendSdkClient(): void {
  aiotBackendSdkClient = null;
}
