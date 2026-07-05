import type { SdkworkAiotAppClient } from '@sdkwork/aiot-app-sdk';

import {
  createAiotCommandService,
  type AiotCommandService,
} from '../command/command-service';
import type { AiotVoiceDialoguePort } from '../ports/dialogue-ports';
import { loadAllDevicePages } from '../device/device-pagination';
import type { AiotVoiceDevice } from '../types/conversation';
import { readRecord, readString } from '../utils/session';

export interface CreateAiotVoiceServiceOptions {
  aiotClient: SdkworkAiotAppClient;
  commandService?: AiotCommandService;
  speechRecognition?: SpeechRecognitionLike | null;
  speechSynthesis?: SpeechSynthesis | null;
  voiceDialoguePort?: AiotVoiceDialoguePort;
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

const DEFAULT_SPEECH_RECOGNITION_LANG = 'zh-CN';
const MAX_MEDIA_RECORDING_MS = 30_000;

export interface StartListeningOptions {
  /** Maximum MediaRecorder capture duration before auto-stop (sdkwork-voice STT path). */
  maxRecordingMs?: number;
}

export interface AiotVoiceService {
  isCloudVoiceConfigured(): boolean;
  isListening(): boolean;
  listVoiceDevices(): Promise<AiotVoiceDevice[]>;
  speakOnDevice(deviceId: string, text: string, sessionId?: string): Promise<void>;
  speakLocally(text: string, lang?: string): Promise<void>;
  speakViaCloud(text: string, options?: { model?: string; voice?: string }): Promise<void>;
  startListening(
    onResult: (text: string, isFinal: boolean) => void,
    options?: StartListeningOptions,
  ): Promise<void>;
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

async function playAudioUrl(url: string): Promise<void> {
  if (typeof window === 'undefined') {
    return;
  }

  await new Promise<void>((resolve, reject) => {
    const audio = new Audio(url);
    audio.onended = () => resolve();
    audio.onerror = () => reject(new Error('Failed to play synthesized audio.'));
    void audio.play().catch(reject);
  });
}

export function createAiotVoiceService(
  options: CreateAiotVoiceServiceOptions,
): AiotVoiceService {
  const commandService = options.commandService ?? createAiotCommandService(options);
  const voiceDialoguePort = options.voiceDialoguePort;
  const speechRecognition = options.speechRecognition ?? resolveBrowserSpeechRecognition();
  const speechSynthesis = options.speechSynthesis
    ?? (typeof window !== 'undefined' ? window.speechSynthesis : null);

  let listening = false;
  let activeRecognition: SpeechRecognitionLike | null = null;

  return {
    isCloudVoiceConfigured() {
      return Boolean(voiceDialoguePort?.configured);
    },

    isListening() {
      return listening;
    },

    async listVoiceDevices() {
      const devices = await loadAllDevicePages(options.aiotClient);
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

    async speakViaCloud(text, synthesisOptions) {
      if (!voiceDialoguePort?.configured) {
        throw new Error('sdkwork-voice dialogue port is not configured.');
      }

      const result = await voiceDialoguePort.synthesize(text, synthesisOptions);
      if (result.audioUrl) {
        await playAudioUrl(result.audioUrl);
        return;
      }

      await this.speakLocally(text);
    },

    async startListening(onResult, listenOptions = {}) {
      if (voiceDialoguePort?.transcribe && typeof MediaRecorder !== 'undefined' && navigator.mediaDevices) {
        try {
          const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
          const recorder = new MediaRecorder(stream);
          const chunks: Blob[] = [];
          const maxRecordingMs = listenOptions.maxRecordingMs ?? MAX_MEDIA_RECORDING_MS;
          let recordingTimer: ReturnType<typeof setTimeout> | undefined;

          listening = true;
          activeRecognition = null;

          recorder.ondataavailable = (event) => {
            if (event.data.size > 0) {
              chunks.push(event.data);
            }
          };

          recorder.onstop = async () => {
            if (recordingTimer) {
              clearTimeout(recordingTimer);
            }
            listening = false;
            stream.getTracks().forEach((track) => track.stop());
            if (chunks.length === 0) {
              onResult('', true);
              return;
            }

            try {
              const blob = new Blob(chunks, { type: recorder.mimeType || 'audio/webm' });
              const result = await voiceDialoguePort.transcribe!({
                audioBlob: blob,
                fileName: 'aiot-input.webm',
                language: 'zh',
              });
              onResult(result.text, true);
            } catch (error) {
              const message = error instanceof Error ? error.message : '语音识别失败';
              onResult(message, true);
            }
          };

          recorder.start();
          recordingTimer = setTimeout(() => {
            if (recorder.state === 'recording') {
              recorder.stop();
            }
          }, maxRecordingMs);

          activeRecognition = {
            abort: () => recorder.stop(),
            addEventListener: () => undefined,
            continuous: false,
            interimResults: false,
            lang: DEFAULT_SPEECH_RECOGNITION_LANG,
            removeEventListener: () => undefined,
            start: () => undefined,
            stop: () => recorder.stop(),
          };
          return;
        } catch (error) {
          const message = error instanceof Error ? error.message : '麦克风不可用';
          onResult(message, true);
          return;
        }
      }

      if (!speechRecognition || listening) {
        return;
      }

      listening = true;
      activeRecognition = speechRecognition;
      speechRecognition.continuous = false;
      speechRecognition.interimResults = true;
      speechRecognition.lang = DEFAULT_SPEECH_RECOGNITION_LANG;

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

      const handleError = () => {
        listening = false;
        activeRecognition = null;
        onResult('浏览器语音识别失败', true);
      };

      const handleEnd = () => {
        listening = false;
        activeRecognition = null;
      };

      speechRecognition.addEventListener('result', handleResult);
      speechRecognition.addEventListener('error', handleError);
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
