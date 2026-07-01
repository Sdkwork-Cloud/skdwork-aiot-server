import { readPcReactRuntimeSession } from '@sdkwork/core-pc-react';
import {
  createDriveAppClient,
  type SdkworkDriveAppClient,
  type SdkworkAppConfig,
} from '@sdkwork/drive-app-sdk';
import { readOptionalBearerToken } from '@sdkwork/aiot-app-core';

import { getAiotPcTokenManager } from './pcTokenManager';
import { resolveDriveAppApiBaseUrl } from './sdkBaseUrls';

export type SdkworkAiotPcDriveClientConfig = SdkworkAppConfig;

let driveAppSdkClient: SdkworkDriveAppClient | null = null;

export function createDriveAppSdkClientConfig(
  session = readPcReactRuntimeSession(),
): SdkworkAiotPcDriveClientConfig {
  return {
    baseUrl: resolveDriveAppApiBaseUrl(),
    authToken: readOptionalBearerToken(session.authToken),
    accessToken: readOptionalBearerToken(session.accessToken),
    platform: 'pc',
  };
}

export function initDriveAppSdkClient(
  config: SdkworkAiotPcDriveClientConfig = createDriveAppSdkClientConfig(),
): SdkworkDriveAppClient {
  const client = createDriveAppClient(config);
  client.setTokenManager(getAiotPcTokenManager());
  driveAppSdkClient = client;
  return driveAppSdkClient;
}

export function getDriveAppSdkClient(): SdkworkDriveAppClient {
  return driveAppSdkClient ?? initDriveAppSdkClient();
}

export function resetDriveAppSdkClient(): void {
  driveAppSdkClient = null;
}
