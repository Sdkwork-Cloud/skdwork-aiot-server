import {
  createAiotAgentService,
  createAiotVoiceDialogueService,
  createAiotVoiceService,
  type AiotAgentService,
  type AiotVoiceDialogueService,
  type AiotVoiceService,
} from '@sdkwork/aiot-app-core';

import {
  createAiotAgentsDialoguePort,
  createAiotVoiceDialoguePort,
  getAiotAppSdkClient,
} from '@sdkwork/aiot-pc-core';

export type {
  AiotVoiceDialogueCatalog,
} from '@sdkwork/aiot-app-core';

export interface CreateSdkworkVoiceServiceOptions {
  agentService?: AiotAgentService;
  voiceService?: AiotVoiceService;
}

export type SdkworkVoiceCatalog = Awaited<ReturnType<AiotVoiceDialogueService['getCatalog']>>;
export type SdkworkVoiceServicePort = AiotVoiceDialogueService;

export function createSdkworkVoiceService(
  options: CreateSdkworkVoiceServiceOptions = {},
): SdkworkVoiceServicePort {
  const agentsDialoguePort = createAiotAgentsDialoguePort();
  const voiceDialoguePort = createAiotVoiceDialoguePort();
  const agentService = options.agentService ?? createAiotAgentService({
    agentsDialoguePort,
    aiotClient: getAiotAppSdkClient(),
  });
  const voiceService = options.voiceService ?? createAiotVoiceService({
    aiotClient: getAiotAppSdkClient(),
    voiceDialoguePort,
  });

  return createAiotVoiceDialogueService({
    agentService,
    agentsDialoguePort,
    voiceDialoguePort,
    voiceService,
  });
}

export const sdkworkVoiceService = createSdkworkVoiceService();
