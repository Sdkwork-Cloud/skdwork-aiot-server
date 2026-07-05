import type { AiotAgentService } from '../agent/agent-service';
import type { AiotAgentsDialoguePort, AiotVoiceDialoguePort } from '../ports/dialogue-ports';
import type { AiotVoiceDevice } from '../types/conversation';
import type { AiotVoiceService } from './voice-service';

export interface AiotVoiceDialogueCatalog {
  agentsConfigured: boolean;
  devices: AiotVoiceDevice[];
  isListening: boolean;
  lastAssistantReply: string;
  selectedDeviceId: string | null;
  transcript: string;
  voiceConfigured: boolean;
}

export interface CreateAiotVoiceDialogueServiceOptions {
  agentService: AiotAgentService;
  agentsDialoguePort?: AiotAgentsDialoguePort;
  voiceDialoguePort?: AiotVoiceDialoguePort;
  voiceService: AiotVoiceService;
}

export interface AiotVoiceDialogueService {
  getCatalog(): Promise<AiotVoiceDialogueCatalog>;
  runDialogueTurn(text: string): Promise<string>;
  selectDevice(deviceId: string | null): void;
  speakSelected(text: string): Promise<void>;
  startListening(onTranscript: (text: string) => void): Promise<void>;
  stopListening(): void;
}

export function createAiotVoiceDialogueService(
  options: CreateAiotVoiceDialogueServiceOptions,
): AiotVoiceDialogueService {
  const { agentService, voiceService } = options;
  const agentsDialoguePort = options.agentsDialoguePort;
  const voiceDialoguePort = options.voiceDialoguePort;

  let selectedDeviceId: string | null = null;
  let transcript = '';
  let lastAssistantReply = '';
  let dialogueSessionId: string | null = null;

  function resolveDialogueDeviceId(): string {
    if (selectedDeviceId) {
      return selectedDeviceId;
    }
    return 'voice-dialogue';
  }

  async function speakReply(text: string): Promise<void> {
    if (voiceDialoguePort?.configured) {
      await voiceService.speakViaCloud(text);
      return;
    }

    if (selectedDeviceId) {
      await voiceService.speakOnDevice(selectedDeviceId, text, dialogueSessionId ?? undefined);
      return;
    }

    await voiceService.speakLocally(text);
  }

  return {
    async getCatalog() {
      const devices = await voiceService.listVoiceDevices();
      if (!selectedDeviceId && devices[0]) {
        selectedDeviceId = devices[0].deviceId;
      }

      return {
        agentsConfigured: Boolean(agentsDialoguePort?.configured),
        devices,
        isListening: voiceService.isListening(),
        lastAssistantReply,
        selectedDeviceId,
        transcript,
        voiceConfigured: Boolean(voiceDialoguePort?.configured),
      };
    },

    async runDialogueTurn(text) {
      const normalized = text.trim();
      if (!normalized) {
        throw new Error('请输入或说出有效内容后再发起对话。');
      }

      if (!dialogueSessionId) {
        const session = agentService.createSession(resolveDialogueDeviceId(), 'AIoT 语音对话');
        dialogueSessionId = session.id;
      }

      const reply = await agentService.sendMessage({
        deviceId: resolveDialogueDeviceId(),
        sessionId: dialogueSessionId,
        text: normalized,
      });

      lastAssistantReply = reply.content;
      await speakReply(reply.content);
      return reply.content;
    },

    selectDevice(deviceId) {
      selectedDeviceId = deviceId;
    },

    async speakSelected(text) {
      await speakReply(text);
    },

    async startListening(onTranscript) {
      transcript = '';
      await voiceService.startListening((value, isFinal) => {
        transcript = value;
        if (isFinal) {
          onTranscript(value);
        }
      });
    },

    stopListening() {
      voiceService.stopListening();
    },
  };
}
