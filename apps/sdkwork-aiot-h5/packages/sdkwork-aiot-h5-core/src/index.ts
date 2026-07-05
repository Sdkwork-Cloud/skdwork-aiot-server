import {
  createAiotAppClient,
  type SdkworkAiotAppClient,
  type SdkworkAiotAppClientConfig,
} from '@sdkwork/aiot-app-sdk';
import { readImportMetaEnvWithDefault } from '@sdkwork/aiot-app-core';

import { readH5RuntimeSession, type H5RuntimeSession } from './sdk/h5RuntimeSession';
import { getAiotH5TokenManager, syncH5TokenManagerFromRuntimeSession } from './sdk/h5TokenManager';

let aiotAppSdkClient: SdkworkAiotAppClient | null = null;

export function createAiotH5AppSdkClientConfig(
  session: H5RuntimeSession = readH5RuntimeSession(),
): SdkworkAiotAppClientConfig {
  return {
    accessToken: session.accessToken,
    authToken: session.authToken,
    baseUrl: readImportMetaEnvWithDefault(
      'VITE_SDKWORK_AIOT_APPLICATION_APP_HTTP_URL',
      'http://127.0.0.1:18082',
    ),
    platform: 'h5',
  };
}

export function initAiotH5AppSdkClient(
  config: SdkworkAiotAppClientConfig = createAiotH5AppSdkClientConfig(),
): SdkworkAiotAppClient {
  const client = createAiotAppClient(config);
  client.setTokenManager(getAiotH5TokenManager());
  aiotAppSdkClient = client;
  return aiotAppSdkClient;
}

export function getAiotH5AppSdkClient(): SdkworkAiotAppClient {
  return aiotAppSdkClient ?? initAiotH5AppSdkClient();
}

export { AiotH5AuthGate, type AiotH5AuthGateProps } from './auth/AiotH5AuthGate';
export {
  getAiotH5TokenManager,
  resetAiotH5TokenManager,
  syncH5TokenManagerFromRuntimeSession,
} from './sdk/h5TokenManager';
export { readH5RuntimeSession, type H5RuntimeSession } from './sdk/h5RuntimeSession';

export {
  createAiotAgentService,
  createAiotCommandService,
  createAiotVoiceService,
  listDevicePage,
  loadAllDevicePages,
  readDeviceId,
} from '@sdkwork/aiot-app-core';
