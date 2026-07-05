import {
  createAiotAgentsDialoguePortFromSdk,
  createAiotVoiceDialoguePortFromSdk,
  type AiotAgentsDialoguePort,
  type AiotVoiceDialoguePort,
} from '@sdkwork/aiot-app-core';

import {
  getAgentsAppSdkClient,
  isAgentsAppSdkConfigured,
  sendAgentChatMessageSync,
} from '../sdk/agentsAppSdkClient';
import { resolveDefaultAiotAgentId } from '../sdk/siblingAppUrls';
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
      return client().voice.transcriptions.create(input);
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
