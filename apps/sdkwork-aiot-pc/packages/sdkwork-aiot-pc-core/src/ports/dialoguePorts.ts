import {
  createAiotAgentsDialoguePortFromSdk,
  createAiotVoiceDialoguePortFromSdk,
  type AiotAgentsDialoguePort,
  type AiotVoiceDialoguePort,
} from '@sdkwork/aiot-app-core';
import type { MediaKind, MediaSource } from '@sdkwork/voice-app-sdk';

import {
  getAgentsAppSdkClient,
  isAgentsAppSdkConfigured,
  sendAgentChatMessageSync,
} from '../sdk/agentsAppSdkClient';
import { resolveDefaultAiotAgentId } from '../sdk/topologyEnvKeys';
import { getVoiceAppSdkClient, isVoiceAppSdkConfigured } from '../sdk/voiceAppSdkClient';

export function createAiotAgentsDialoguePort(): AiotAgentsDialoguePort {
  return createAiotAgentsDialoguePortFromSdk({
    configured: isAgentsAppSdkConfigured(),
    resolveAgentId: resolveDefaultAiotAgentId,
    createSession(agentId, input) {
      return getAgentsAppSdkClient().ai.agents.sessions.create(agentId, input);
    },
    sendChatSync(agentId, remoteSessionId, input) {
      return sendAgentChatMessageSync(getAgentsAppSdkClient(), agentId, remoteSessionId, input);
    },
  });
}

export function createAiotVoiceDialoguePort(): AiotVoiceDialoguePort {
  const client = () => getVoiceAppSdkClient();
  return createAiotVoiceDialoguePortFromSdk({
    configured: isVoiceAppSdkConfigured(),
    createSpeech(input) {
      return client().voice.speech.create(input);
    },
    createTranscription(input) {
      return client().voice.transcriptions.create({
        file: {
          fileName: input.file.fileName,
          kind: input.file.kind as MediaKind,
          mimeType: input.file.mimeType,
          source: input.file.source as MediaSource,
          url: input.file.url,
        },
        language: input.language,
        model: input.model,
        responseFormat: input.responseFormat as 'json',
      });
    },
    listAudioAssets(input) {
      return client().voice.audioAssets.list(input);
    },
    retrieveAudioAsset(assetId) {
      return client().voice.audioAssets.retrieve(assetId);
    },
    retrieveTask(taskId) {
      return client().voice.tasks.retrieve(taskId);
    },
  });
}
