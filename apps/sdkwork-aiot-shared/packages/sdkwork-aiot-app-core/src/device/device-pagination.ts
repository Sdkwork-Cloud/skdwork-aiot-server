import type { AiotDevice, SdkworkAiotAppClient } from '@sdkwork/aiot-app-sdk';

import { readRecord, readString } from '../utils/session';

export interface ListDevicePageParams {
  page?: number;
  page_size?: number;
}

export interface ListDevicePageResult {
  hasMore: boolean;
  items: AiotDevice[];
  page: number;
  pageSize: number;
}

export async function listDevicePage(
  aiotClient: SdkworkAiotAppClient,
  params: ListDevicePageParams = {},
): Promise<ListDevicePageResult> {
  const page = params.page ?? 1;
  const pageSize = params.page_size ?? 20;
  const response = await aiotClient.iot.devices.list({ page, page_size: pageSize });
  const items = Array.isArray(response.items) ? response.items : [];
  const pageInfo = readRecord(response.pageInfo);

  return {
    hasMore: Boolean(pageInfo.hasMore),
    items,
    page: typeof pageInfo.page === 'number' ? pageInfo.page : page,
    pageSize: typeof pageInfo.pageSize === 'number' ? pageInfo.pageSize : pageSize,
  };
}

/** Device picker only: loads every page from the authoritative store. */
export async function loadAllDevicePages(
  aiotClient: SdkworkAiotAppClient,
  pageSize = 200,
): Promise<AiotDevice[]> {
  const devices: AiotDevice[] = [];
  let page = 1;

  while (true) {
    const result = await listDevicePage(aiotClient, { page, page_size: pageSize });
    devices.push(...result.items);
    if (!result.hasMore || result.items.length === 0) {
      break;
    }
    page += 1;
  }

  return devices;
}

export function readDeviceId(device: AiotDevice | Record<string, unknown>): string {
  const record = readRecord(device);
  return readString(record.deviceId) || readString(record.id);
}
