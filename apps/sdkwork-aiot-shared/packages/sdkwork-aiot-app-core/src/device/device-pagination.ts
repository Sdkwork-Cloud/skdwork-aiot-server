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
  const items = (Array.isArray(response.items) ? response.items : []) as unknown as AiotDevice[];
  const pageInfo = readRecord(response.pageInfo);

  return {
    hasMore: Boolean(pageInfo.hasMore),
    items,
    page: typeof pageInfo.page === 'number' ? pageInfo.page : page,
    pageSize: typeof pageInfo.pageSize === 'number' ? pageInfo.pageSize : pageSize,
    total: typeof pageInfo.total === 'number' ? pageInfo.total : undefined,
  };
}

/** Loads device pages from the server for pickers (bounded, no client-side slice pagination). */
export async function listDevicePagesForPicker(
  aiotClient: SdkworkAiotAppClient,
  options: { maxPages?: number } = {},
): Promise<AiotDevice[]> {
  const maxPages = options.maxPages ?? 10;
  const collected: AiotDevice[] = [];
  let page = 1;
  let hasMore = true;

  while (hasMore && page <= maxPages) {
    const result = await listDevicePage(aiotClient, { page });
    collected.push(...result.items);
    hasMore = result.hasMore;
    page += 1;
  }

  return collected;
}

export function readDeviceId(device: AiotDevice | Record<string, unknown>): string {
  const record = readRecord(device);
  return readString(record.deviceId) || readString(record.id);
}
