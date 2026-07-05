import type { AiotDevice, SdkworkAiotAppClient } from '@sdkwork/aiot-app-sdk';

import { readRecord, readString } from '../utils/session';

export const DEFAULT_DEVICE_LIST_PAGE_SIZE = 20;

export interface ListDevicePageParams {
  page?: number;
  pageSize?: number;
}

export interface ListDevicePageResult {
  hasMore: boolean;
  items: AiotDevice[];
  page: number;
  pageSize: number;
  total?: number;
}

export async function listDevicePage(
  aiotClient: SdkworkAiotAppClient,
  params: ListDevicePageParams = {},
): Promise<ListDevicePageResult> {
  const page = params.page ?? 1;
  const pageSize = params.pageSize ?? DEFAULT_DEVICE_LIST_PAGE_SIZE;
  const response = await aiotClient.iot.devices.list({ page, pageSize });
  const items = Array.isArray(response.items) ? response.items : [];
  const pageInfo = readRecord(response.pageInfo);

  return {
    hasMore: Boolean(pageInfo.hasMore),
    items,
    page: typeof pageInfo.page === 'number' ? pageInfo.page : page,
    pageSize: typeof pageInfo.pageSize === 'number' ? pageInfo.pageSize : pageSize,
    total: typeof pageInfo.total === 'number' ? pageInfo.total : undefined,
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
    const result = await listDevicePage(aiotClient, { page, pageSize });
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
