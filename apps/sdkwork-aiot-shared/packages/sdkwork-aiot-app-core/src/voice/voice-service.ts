import type { SdkworkAiotAppClient } from '@sdkwork/aiot-app-sdk';

import {
  createAiotCommandService,
  type AiotCommandService,
} from '../command/command-service';
import type { AiotVoiceDevice } from '../types/conversation';
import { readRecord, readString } from '../utils/session';

export interface CreateAiotVoiceServiceOptions {
  aiotClient: SdkworkAiotAppClient;
  commandService?: AiotCommandService;
  speechRecognition?: SpeechRecognitionLike | null;
  speechSynthesis?: SpeechSynthesis | null;
}

export interface SpeechRecognitionLike {
  abort(): void;
  addEventListener(type: 'end' | 'error' | 'result', listener: EventListener): void;
  continuous: boolean;
  interimResults: boolean;
  lang: string;
  removeEventListener(type: 'end' | 'error' | 'result', listener: EventListener): void;
  start(): void;
  stop(): void;
}

export interface AiotVoiceService {
  isListening(): boolean;
  listVoiceDevices(): Promise<AiotVoiceDevice[]>;
  speakOnDevice(deviceId: string, text: string, sessionId?: string): Promise<void>;
  speakLocally(text: string, lang?: string): Promise<void>;
  startListening(onResult: (text: string, isFinal: boolean) => void): Promise<void>;
  stopListening(): void;
}

function mapVoiceDevice(device: Record<string, unknown>): AiotVoiceDevice {
  const metadata = readRecord(device.metadata);
  const deviceId = readString(device.deviceId) || readString(device.id);
  const chipFamily = readString(device.chipFamily) || readString(metadata.chipFamily) || undefined;
  const productId = readString(device.productId) || undefined;
  const status = readString(device.status, 'offline');

  return {
    chipFamily,
    deviceId,
    displayName: readString(device.displayName, deviceId),
    online: status.toLowerCase() === 'online',
    productId,
    status,
  };
}

function resolveBrowserSpeechRecognition(): SpeechRecognitionLike | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const candidate = (window as Window & {
    SpeechRecognition?: new () => SpeechRecognitionLike;
    webkitSpeechRecognition?: new () => SpeechRecognitionLike;
  }).SpeechRecognition
    ?? (window as Window & { webkitSpeechRecognition?: new () => SpeechRecognitionLike }).webkitSpeechRecognition;

  return candidate ? new candidate() : null;
}

export function createAiotVoiceService(
  options: CreateAiotVoiceServiceOptions,
): AiotVoiceService {
  const commandService = options.commandService ?? createAiotCommandService(options);
  const speechRecognition = options.speechRecognition ?? resolveBrowserSpeechRecognition();
  const speechSynthesis = options.speechSynthesis
    ?? (typeof window !== 'undefined' ? window.speechSynthesis : null);

  let listening = false;
  let activeRecognition: SpeechRecognitionLike | null = null;

  return {
    isListening() {
      return listening;
    },

    async listVoiceDevices() {
      const page = await options.aiotClient.iot.devices.list();
      const devices = Array.isArray(page.items) ? page.items : [];
      return devices
        .map((device) => mapVoiceDevice(readRecord(device)))
        .filter((device) => device.deviceId.length > 0);
    },

    async speakOnDevice(deviceId, text, sessionId) {
      await commandService.speak(deviceId, text, sessionId);
    },

    async speakLocally(text, lang = 'zh-CN') {
      if (!speechSynthesis || typeof SpeechSynthesisUtterance === 'undefined') {
        return;
      }

      await new Promise<void>((resolve) => {
        const utterance = new SpeechSynthesisUtterance(text);
        utterance.lang = lang;
        utterance.onend = () => resolve();
        utterance.onerror = () => resolve();
        speechSynthesis.speak(utterance);
      });
    },

    async startListening(onResult) {
      if (!speechRecognition || listening) {
        return;
      }

      listening = true;
      activeRecognition = speechRecognition;
      speechRecognition.continuous = false;
      speechRecognition.interimResults = true;
      speechRecognition.lang = 'zh-CN';

      const handleResult = (event: Event) => {
        const resultEvent = event as Event & {
          results: ArrayLike<{ 0: { transcript: string }; isFinal: boolean }>;
        };

        const latest = resultEvent.results[resultEvent.results.length - 1];
        if (!latest) {
          return;
        }

        onResult(latest[0].transcript, latest.isFinal);
      };

      const handleEnd = () => {
        listening = false;
        activeRecognition = null;
      };

      speechRecognition.addEventListener('result', handleResult);
      speechRecognition.addEventListener('end', handleEnd);
      speechRecognition.start();
    },

    stopListening() {
      if (!activeRecognition) {
        listening = false;
        return;
      }

      activeRecognition.stop();
      listening = false;
      activeRecognition = null;
    },
  };
}
