import { readPcReactRuntimeSession } from '@sdkwork/core-pc-react';
import {
  createAiotAppClient,
  type SdkworkAiotAppClient,
  type SdkworkAiotAppClientConfig,
} from '@sdkwork/aiot-app-sdk';
import { readOptionalBearerToken } from '@sdkwork/aiot-app-core';

import { getAiotPcTokenManager } from './pcTokenManager';
import { resolveAiotAppApiBaseUrl } from './sdkBaseUrls';

export type SdkworkAiotPcAppClientConfig = SdkworkAiotAppClientConfig;

let aiotAppSdkClient: SdkworkAiotAppClient | null = null;

export function createAiotAppSdkClientConfig(
  session = readPcReactRuntimeSession(),
): SdkworkAiotPcAppClientConfig {
  return {
    baseUrl: resolveAiotAppApiBaseUrl(),
    authToken: readOptionalBearerToken(session.authToken),
    accessToken: readOptionalBearerToken(session.accessToken),
    platform: 'pc',
  };
}

export function initAiotAppSdkClient(
  config: SdkworkAiotPcAppClientConfig = createAiotAppSdkClientConfig(),
): SdkworkAiotAppClient {
  const client = createAiotAppClient(config);
  client.setTokenManager(getAiotPcTokenManager());
  aiotAppSdkClient = client;
  return aiotAppSdkClient;
}

export function getAiotAppSdkClient(): SdkworkAiotAppClient {
  return aiotAppSdkClient ?? initAiotAppSdkClient();
}

export function resetAiotAppSdkClient(): void {
  aiotAppSdkClient = null;
}
