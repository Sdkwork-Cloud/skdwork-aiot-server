import { getAiotAppSdkClient } from '@sdkwork/aiot-pc-core';
import {
  createAiotVoiceService,
  type AiotVoiceDevice,
  type AiotVoiceService,
} from '@sdkwork/aiot-app-core';

export interface CreateSdkworkVoiceServiceOptions {
  voiceService?: AiotVoiceService;
}

export interface SdkworkVoiceCatalog {
  devices: AiotVoiceDevice[];
  isListening: boolean;
  selectedDeviceId: string | null;
  transcript: string;
}

export interface SdkworkVoiceServicePort {
  getCatalog(): Promise<SdkworkVoiceCatalog>;
  selectDevice(deviceId: string | null): void;
  speakSelected(text: string): Promise<void>;
  startListening(onTranscript: (text: string) => void): Promise<void>;
  stopListening(): void;
}

export function createSdkworkVoiceService(
  options: CreateSdkworkVoiceServiceOptions = {},
): SdkworkVoiceServicePort {
  const voiceService = options.voiceService ?? createAiotVoiceService({
    aiotClient: getAiotAppSdkClient(),
  });

  let selectedDeviceId: string | null = null;
  let transcript = '';

  return {
    async getCatalog() {
      const devices = await voiceService.listVoiceDevices();
      if (!selectedDeviceId && devices[0]) {
        selectedDeviceId = devices[0].deviceId;
      }

      return {
        devices,
        isListening: voiceService.isListening(),
        selectedDeviceId,
        transcript,
      };
    },

    selectDevice(deviceId) {
      selectedDeviceId = deviceId;
    },

    async speakSelected(text) {
      if (!selectedDeviceId) {
        await voiceService.speakLocally(text);
        return;
      }

      await voiceService.speakOnDevice(selectedDeviceId, text);
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

export const sdkworkVoiceService = createSdkworkVoiceService();
