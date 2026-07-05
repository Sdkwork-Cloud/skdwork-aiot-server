import type { AiotAgentService } from '../agent/agent-service';
import type { AiotAgentsDialoguePort, AiotVoiceDialoguePort } from '../ports/dialogue-ports';
import type { AiotVoiceDevice } from '../types/conversation';
import type { AiotVoiceService } from './voice-service';

export interface AiotVoiceDialogueCatalog {
  agentsConfigured: boolean;
  devices: AiotVoiceDevice[];
  isListening: boolean;
  isSpeaking: boolean;
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

export interface RunDialogueTurnOptions {
  /** When true (default), synthesize or speak the assistant reply after chat completes. */
  speakReply?: boolean;
}

export interface AiotVoiceDialogueService {
  getCatalog(): Promise<AiotVoiceDialogueCatalog>;
  runDialogueTurn(text: string, options?: RunDialogueTurnOptions): Promise<string>;
  selectDevice(deviceId: string | null): void;
  speakSelected(text: string): Promise<void>;
  startListening(
    onTranscript: (text: string, isFinal: boolean) => void,
    options?: { autoRunDialogue?: boolean; maxRecordingMs?: number },
  ): Promise<void>;
  stopListening(): void;
}

export function createAiotVoiceDialogueService(
  options: CreateAiotVoiceDialogueServiceOptions,
): AiotVoiceDialogueService {
  const { agentService, voiceService } = options;
  const agentsDialoguePort = options.agentsDialoguePort;
  const voiceDialoguePort = options.voiceDialoguePort;

  let selectedDeviceId: string | null = null;
  let cachedDevices: AiotVoiceDevice[] = [];
  let transcript = '';
  let lastAssistantReply = '';
  let dialogueSessionId: string | null = null;
  let isSpeaking = false;

  function resolveDialogueDeviceId(): string {
    if (selectedDeviceId) {
      return selectedDeviceId;
    }
    return 'voice-dialogue';
  }

  function resolveSelectedDevice(): AiotVoiceDevice | undefined {
    if (!selectedDeviceId) {
      return undefined;
    }
    return cachedDevices.find((device) => device.deviceId === selectedDeviceId);
  }

  async function refreshVoiceDevices(): Promise<void> {
    cachedDevices = await voiceService.listVoiceDevices();
    if (!selectedDeviceId && cachedDevices[0]) {
      selectedDeviceId = cachedDevices[0].deviceId;
    }
  }

  async function speakReply(text: string): Promise<void> {
    const normalized = text.trim();
    if (!normalized) {
      return;
    }

    if (cachedDevices.length === 0) {
      await refreshVoiceDevices();
    }

    isSpeaking = true;
    try {
      const selectedDevice = resolveSelectedDevice();

      // Prefer on-device playback when the selected hardware is online.
      if (selectedDevice?.online) {
        await voiceService.speakOnDevice(
          selectedDevice.deviceId,
          normalized,
          dialogueSessionId ?? undefined,
        );
        return;
      }

      if (voiceDialoguePort?.configured) {
        await voiceService.speakViaCloud(normalized);
        return;
      }

      if (selectedDeviceId) {
        await voiceService.speakOnDevice(
          selectedDeviceId,
          normalized,
          dialogueSessionId ?? undefined,
        );
        return;
      }

      await voiceService.speakLocally(normalized);
    } finally {
      isSpeaking = false;
    }
  }

  async function runDialogueTurnInternal(
    text: string,
    runOptions: RunDialogueTurnOptions = {},
  ): Promise<string> {
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
    if (runOptions.speakReply !== false) {
      await speakReply(reply.content);
    }
    return reply.content;
  }

  async function getCatalogInternal(): Promise<AiotVoiceDialogueCatalog> {
    await refreshVoiceDevices();

    return {
      agentsConfigured: Boolean(agentsDialoguePort?.configured),
      devices: cachedDevices,
      isListening: voiceService.isListening(),
      isSpeaking,
      lastAssistantReply,
      selectedDeviceId,
      transcript,
      voiceConfigured: Boolean(voiceDialoguePort?.configured),
    };
  }

  return {
    getCatalog() {
      return getCatalogInternal();
    },

    runDialogueTurn(text, runOptions = {}) {
      return runDialogueTurnInternal(text, runOptions);
    },

    selectDevice(deviceId) {
      selectedDeviceId = deviceId;
    },

    speakSelected(text) {
      return speakReply(text);
    },

    async startListening(onTranscript, listenOptions = {}) {
      transcript = '';
      await voiceService.startListening(async (value, isFinal) => {
        transcript = value;
        onTranscript(value, isFinal);

        if (isFinal && listenOptions.autoRunDialogue && value.trim()) {
          try {
            await runDialogueTurnInternal(value.trim());
          } catch (error) {
            onTranscript(
              error instanceof Error ? error.message : '语音对话失败',
              true,
            );
          }
        }
      }, {
        maxRecordingMs: listenOptions.maxRecordingMs,
      });
    },

    stopListening() {
      voiceService.stopListening();
    },
  };
}
