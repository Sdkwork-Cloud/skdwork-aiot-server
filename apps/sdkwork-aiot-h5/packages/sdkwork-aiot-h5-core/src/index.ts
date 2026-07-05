import {
  createAiotAppClient,
  type SdkworkAiotAppClient,
  type SdkworkAiotAppClientConfig,
} from '@sdkwork/aiot-app-sdk';
import {
  createAiotAgentService,
  createAiotCommandService,
  createAiotVoiceDialogueService,
  createAiotVoiceService,
  listDevicePage,
  loadAllDevicePages,
  readDeviceId,
  type AiotAgentService,
  type AiotVoiceDialogueService,
} from '@sdkwork/aiot-app-core';
import { readImportMetaEnvWithDefault } from '@sdkwork/aiot-app-core';

import { createAiotAgentsDialoguePort, createAiotVoiceDialoguePort } from './ports/dialoguePorts';
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

export function createAiotH5VoiceDialogueService(): AiotVoiceDialogueService {
  const agentsDialoguePort = createAiotAgentsDialoguePort();
  const voiceDialoguePort = createAiotVoiceDialoguePort();
  const aiotClient = getAiotH5AppSdkClient();
  const agentService = createAiotAgentService({ agentsDialoguePort, aiotClient });
  const voiceService = createAiotVoiceService({ aiotClient, voiceDialoguePort });
  return createAiotVoiceDialogueService({
    agentService,
    agentsDialoguePort,
    voiceDialoguePort,
    voiceService,
  });
}

export function createAiotH5AgentService(): AiotAgentService {
  return createAiotAgentService({
    agentsDialoguePort: createAiotAgentsDialoguePort(),
    aiotClient: getAiotH5AppSdkClient(),
  });
}

export { AiotH5AuthGate, type AiotH5AuthGateProps } from './auth/AiotH5AuthGate';
export {
  createAiotAgentsDialoguePort,
  createAiotVoiceDialoguePort,
} from './ports/dialoguePorts';
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
