import {
  extractSdkItems,
  extractSdkResourceRecord,
  pollVoiceTaskUntilTerminal,
  readAssistantMessageText,
  readMediaResourceUrl,
  readString,
  readTranscriptText,
  readVoiceTaskId,
} from '../utils/voice-task-runtime';
import { readImportMetaEnv } from '../utils/runtimeEnv';
import type { AiotAgentsDialoguePort, AiotVoiceDialoguePort } from './dialogue-ports';

export interface AiotAgentsSdkBridge {
  configured: boolean;
  resolveAgentId: () => string;
  createSession(
    agentId: string,
    input: { requestedAt: string; title: string },
  ): Promise<unknown>;
  sendChatSync(
    agentId: string,
    remoteSessionId: string,
    input: { content: string; contentType: string; requestedAt: string },
  ): Promise<unknown>;
}

export interface AiotVoiceSdkBridge {
  configured: boolean;
  createSpeech(input: { input: string; model: string; voice: string }): Promise<unknown>;
  createTranscription(input: {
    file: { fileName: string; kind: string; mimeType: string; sourceUri: string };
    language: string;
    model: string;
    responseFormat: string;
  }): Promise<unknown>;
  listAudioAssets(input: { page: number; pageSize: number; taskId: string }): Promise<unknown>;
  retrieveAudioAsset(assetId: string): Promise<unknown>;
  retrieveTask(taskId: string): Promise<{ errorMessage?: string; status?: string }>;
}

async function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        resolve(reader.result);
        return;
      }
      reject(new Error('Failed to encode audio blob.'));
    };
    reader.onerror = () => reject(new Error('Failed to read audio blob.'));
    reader.readAsDataURL(blob);
  });
}

async function resolveSpeechAudioFromTask(
  bridge: AiotVoiceSdkBridge,
  taskId: string,
): Promise<{ audioUrl?: string; mimeType?: string; taskId: string }> {
  const assets = await bridge.listAudioAssets({ page: 1, pageSize: 10, taskId });
  const firstAsset = extractSdkItems(assets)[0];
  const assetRecord = extractSdkResourceRecord(firstAsset);
  const inlineMedia = readMediaResourceUrl(assetRecord.mediaResource);
  if (inlineMedia.url) {
    return { audioUrl: inlineMedia.url, mimeType: inlineMedia.mimeType, taskId };
  }

  const assetId = readString(assetRecord.id);
  if (assetId) {
    const asset = await bridge.retrieveAudioAsset(assetId);
    const media = readMediaResourceUrl(extractSdkResourceRecord(asset).mediaResource);
    if (media.url) {
      return { audioUrl: media.url, mimeType: media.mimeType, taskId };
    }
  }

  return { taskId };
}

export function createAiotAgentsDialoguePortFromSdk(
  bridge: AiotAgentsSdkBridge,
): AiotAgentsDialoguePort {
  return {
    configured: bridge.configured,

    resolveAgentId(_deviceId) {
      return bridge.resolveAgentId();
    },

    async createRemoteSession(agentId, title) {
      const response = await bridge.createSession(agentId, {
        requestedAt: new Date().toISOString(),
        title: title?.trim() || 'AIoT Voice Session',
      });
      const session = extractSdkResourceRecord(response);
      const sessionId = readString(session.sessionId)
        || readString(session.session_id)
        || readString(session.id);
      if (!sessionId) {
        throw new Error('sdkwork-agents session create did not return sessionId.');
      }
      return sessionId;
    },

    async sendChat({ agentId, remoteSessionId, text }) {
      const response = await bridge.sendChatSync(agentId, remoteSessionId, {
        content: text.trim(),
        contentType: 'text/plain',
        requestedAt: new Date().toISOString(),
      });
      const completion = extractSdkResourceRecord(response);
      const reply = readAssistantMessageText(completion);
      if (!reply) {
        throw new Error('sdkwork-agents chat completion did not return assistantMessage.');
      }
      return reply;
    },
  };
}

export function createAiotVoiceDialoguePortFromSdk(
  bridge: AiotVoiceSdkBridge,
): AiotVoiceDialoguePort {
  return {
    configured: bridge.configured,

    async synthesize(text, options = {}) {
      const normalizedText = text.trim();
      const model = options.model?.trim()
        || readImportMetaEnv('VITE_SDKWORK_AIOT_VOICE_DEFAULT_MODEL')
        || 'tts-1';
      const voice = options.voice?.trim()
        || readImportMetaEnv('VITE_SDKWORK_AIOT_VOICE_DEFAULT_VOICE')
        || 'alloy';

      if (!normalizedText) {
        throw new Error('Voice synthesis requires non-empty text.');
      }

      const created = await bridge.createSpeech({ input: normalizedText, model, voice });
      const taskId = readVoiceTaskId(extractSdkResourceRecord(created));
      if (!taskId) {
        throw new Error('sdkwork-voice speech task did not return task id.');
      }

      return pollVoiceTaskUntilTerminal(taskId, {
        onPoll: async (activeTaskId) => {
          const task = await bridge.retrieveTask(activeTaskId);
          return {
            errorMessage: readString(task.errorMessage),
            status: readString(task.status, 'queued'),
          };
        },
        resolveResult: (activeTaskId) => resolveSpeechAudioFromTask(bridge, activeTaskId),
      });
    },

    async transcribe(input) {
      const model = input.model?.trim()
        || readImportMetaEnv('VITE_SDKWORK_AIOT_VOICE_TRANSCRIPTION_MODEL')
        || readImportMetaEnv('VITE_SDKWORK_AIOT_VOICE_DEFAULT_MODEL')
        || 'whisper-1';
      const dataUrl = await blobToDataUrl(input.audioBlob);
      const created = await bridge.createTranscription({
        file: {
          fileName: input.fileName ?? 'aiot-input.webm',
          kind: 'audio',
          mimeType: input.audioBlob.type || 'audio/webm',
          sourceUri: dataUrl,
        },
        language: input.language ?? 'zh',
        model,
        responseFormat: 'json',
      });

      const taskId = readVoiceTaskId(extractSdkResourceRecord(created));
      if (!taskId) {
        const inlineText = readTranscriptText(extractSdkResourceRecord(created));
        if (inlineText) {
          return { text: inlineText };
        }
        throw new Error('sdkwork-voice transcription task did not return task id.');
      }

      return pollVoiceTaskUntilTerminal(taskId, {
        onPoll: async (activeTaskId) => {
          const task = await bridge.retrieveTask(activeTaskId);
          return {
            errorMessage: readString(task.errorMessage),
            status: readString(task.status, 'queued'),
          };
        },
        resolveResult: async (activeTaskId) => {
          const assets = await bridge.listAudioAssets({ page: 1, pageSize: 10, taskId: activeTaskId });
          const firstAsset = extractSdkItems(assets)[0];
          const assetRecord = extractSdkResourceRecord(firstAsset);
          const transcriptText = readTranscriptText(assetRecord);
          if (transcriptText) {
            return { taskId: activeTaskId, text: transcriptText };
          }
          return null;
        },
      });
    },
  };
}
