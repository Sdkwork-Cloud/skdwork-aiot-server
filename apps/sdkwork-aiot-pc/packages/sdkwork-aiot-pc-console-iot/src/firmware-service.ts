import {
  getAiotBackendSdkClient,
  uploadAiotFirmwareArtifactToDrive,
  type UploadAiotFirmwareArtifactInput,
  type UploadAiotFirmwareArtifactResult,
  type AiotFirmwareArtifact,
} from '@sdkwork/aiot-pc-core';

import { readRecord } from '@sdkwork/aiot-app-core';

export interface SdkworkFirmwareService {
  listArtifacts(): Promise<AiotFirmwareArtifact[]>;
  uploadArtifact(input: UploadAiotFirmwareArtifactInput): Promise<UploadAiotFirmwareArtifactResult>;
}

export interface CreateSdkworkFirmwareServiceOptions {
  listArtifacts?: () => Promise<AiotFirmwareArtifact[]>;
  uploadArtifact?: (input: UploadAiotFirmwareArtifactInput) => Promise<UploadAiotFirmwareArtifactResult>;
}

const DEFAULT_FIRMWARE_LIST_PAGE_SIZE = 20;
const DEFAULT_FIRMWARE_LIST_MAX_PAGES = 50;

async function fetchFirmwareArtifactPages(
  maxPages: number = DEFAULT_FIRMWARE_LIST_MAX_PAGES,
): Promise<AiotFirmwareArtifact[]> {
  const client = getAiotBackendSdkClient();
  const collected: AiotFirmwareArtifact[] = [];
  let page = 1;
  let hasMore = true;

  while (hasMore && page <= maxPages) {
    const response = await client.iot.firmwareArtifacts.list({
      page,
      pageSize: DEFAULT_FIRMWARE_LIST_PAGE_SIZE,
    });
    collected.push(...(response.items ?? []).map((item) => item as unknown as AiotFirmwareArtifact));
    const pageInfo = readRecord(response.pageInfo);
    hasMore = Boolean(pageInfo.hasMore);
    page += 1;
  }

  return collected;
}

export function createSdkworkFirmwareService(
  options: CreateSdkworkFirmwareServiceOptions = {},
): SdkworkFirmwareService {
  return {
    async listArtifacts() {
      if (options.listArtifacts) {
        return options.listArtifacts();
      }
      return fetchFirmwareArtifactPages();
    },
    async uploadArtifact(input) {
      if (options.uploadArtifact) {
        return options.uploadArtifact(input);
      }
      return uploadAiotFirmwareArtifactToDrive(input);
    },
  };
}
