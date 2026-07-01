import {
  getAiotBackendSdkClient,
  uploadAiotFirmwareArtifactToDrive,
  type UploadAiotFirmwareArtifactInput,
  type UploadAiotFirmwareArtifactResult,
} from '@sdkwork/aiot-pc-core';
import type { AiotFirmwareArtifact } from '@sdkwork/aiot-backend-sdk';

export interface SdkworkFirmwareService {
  listArtifacts(): Promise<AiotFirmwareArtifact[]>;
  uploadArtifact(input: UploadAiotFirmwareArtifactInput): Promise<UploadAiotFirmwareArtifactResult>;
}

export interface CreateSdkworkFirmwareServiceOptions {
  listArtifacts?: () => Promise<AiotFirmwareArtifact[]>;
  uploadArtifact?: (input: UploadAiotFirmwareArtifactInput) => Promise<UploadAiotFirmwareArtifactResult>;
}

export function createSdkworkFirmwareService(
  options: CreateSdkworkFirmwareServiceOptions = {},
): SdkworkFirmwareService {
  return {
    async listArtifacts() {
      if (options.listArtifacts) {
        return options.listArtifacts();
      }
      const page = await getAiotBackendSdkClient().iot.firmwareArtifacts.list();
      return (page.items ?? []) as AiotFirmwareArtifact[];
    },
    async uploadArtifact(input) {
      if (options.uploadArtifact) {
        return options.uploadArtifact(input);
      }
      return uploadAiotFirmwareArtifactToDrive(input);
    },
  };
}
