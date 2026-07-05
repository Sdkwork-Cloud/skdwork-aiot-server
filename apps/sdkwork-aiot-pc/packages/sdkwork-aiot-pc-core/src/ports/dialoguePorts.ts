import type {
  AiotAgentsDialoguePort,
  AiotVoiceDialoguePort,
} from '@sdkwork/aiot-app-core';
import {
  extractSdkItems,
  extractSdkResourceRecord,
  pollVoiceTaskUntilTerminal,
  readAssistantMessageText,
  readImportMetaEnv,
  readMediaResourceUrl,
  readString,
  readTranscriptText,
  readVoiceTaskId,
} from '@sdkwork/aiot-app-core';

export function createAiotAgentsDialoguePort(): AiotAgentsDialoguePort {
  return {
    configured: isAgentsAppSdkConfigured(),

    resolveAgentId(_deviceId) {
      return resolveDefaultAiotAgentId();
    },

    async createRemoteSession(agentId, title) {
      const response = await getAgentsAppSdkClient().ai.agents.sessions.create(agentId, {
        title: title?.trim() || 'AIoT Voice Session',
        requestedAt: new Date().toISOString(),
      });
      const session = extractSdkResourceRecord(response);
      const sessionId = readString(session.sessionId) || readString(session.session_id) || readString(session.id);
      if (!sessionId) {
        throw new Error('sdkwork-agents session create did not return sessionId.');
      }
      return sessionId;
    },

    async sendChat({ agentId, remoteSessionId, text }) {
      const response = await sendAgentChatMessageSync(
        getAgentsAppSdkClient(),
        agentId,
        remoteSessionId,
        {
          content: text.trim(),
          contentType: 'text/plain',
          requestedAt: new Date().toISOString(),
        },
      );
      const completion = extractSdkResourceRecord(response);
      const reply = readAssistantMessageText(completion);
      if (!reply) {
        throw new Error('sdkwork-agents chat completion did not return assistantMessage.');
      }
      return reply;
    },
  };
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

export function createAiotVoiceDialoguePort(): AiotVoiceDialoguePort {
  return {
    configured: isVoiceAppSdkConfigured(),

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

      const client = getVoiceAppSdkClient();
      const created = await client.voice.speech.create({
        input: normalizedText,
        model,
        voice,
      });
      const taskId = readVoiceTaskId(extractSdkResourceRecord(created));
      if (!taskId) {
        throw new Error('sdkwork-voice speech task did not return task id.');
      }

      return pollVoiceTaskUntilTerminal(taskId, {
        onPoll: async (activeTaskId) => {
          const task = await client.voice.tasks.retrieve(activeTaskId);
          return {
            status: readString(task.status, 'queued'),
            errorMessage: readString(task.errorMessage),
          };
        },
        resolveResult: async (activeTaskId) => {
          const assets = await client.voice.audioAssets.list({ taskId: activeTaskId, page: 1, pageSize: 10 });
          const firstAsset = extractSdkItems(assets)[0];
          const assetRecord = extractSdkResourceRecord(firstAsset);
          const inlineMedia = readMediaResourceUrl(assetRecord.mediaResource);
          if (inlineMedia.url) {
            return {
              audioUrl: inlineMedia.url,
              mimeType: inlineMedia.mimeType,
              taskId: activeTaskId,
            };
          }

          const assetId = readString(assetRecord.id);
          if (assetId) {
            const asset = await client.voice.audioAssets.retrieve(assetId);
            const media = readMediaResourceUrl(asset.mediaResource);
            if (media.url) {
              return {
                audioUrl: media.url,
                mimeType: media.mimeType,
                taskId: activeTaskId,
              };
            }
          }

          return { taskId: activeTaskId };
        },
      });
    },

    async transcribe(input) {
      const model = input.model?.trim()
        || readImportMetaEnv('VITE_SDKWORK_AIOT_VOICE_TRANSCRIPTION_MODEL')
        || readImportMetaEnv('VITE_SDKWORK_AIOT_VOICE_DEFAULT_MODEL')
        || 'whisper-1';
      const dataUrl = await blobToDataUrl(input.audioBlob);
      const client = getVoiceAppSdkClient();
      const created = await client.voice.transcriptions.create({
        file: {
          kind: 'audio',
          sourceUri: dataUrl,
          fileName: input.fileName ?? 'aiot-input.webm',
          mimeType: input.audioBlob.type || 'audio/webm',
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
          const task = await client.voice.tasks.retrieve(activeTaskId);
          return {
            status: readString(task.status, 'queued'),
            errorMessage: readString(task.errorMessage),
          };
        },
        resolveResult: async (activeTaskId) => {
          const assets = await client.voice.audioAssets.list({ taskId: activeTaskId, page: 1, pageSize: 10 });
          const firstAsset = extractSdkItems(assets)[0];
          const assetRecord = extractSdkResourceRecord(firstAsset);
          const text = readTranscriptText(assetRecord);
          if (text) {
            return { taskId: activeTaskId, text };
          }
          return null;
        },
      });
    },
  };
}
