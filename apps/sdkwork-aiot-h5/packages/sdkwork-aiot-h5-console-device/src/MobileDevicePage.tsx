import { useCallback, useEffect, useState } from 'react';
import { getAiotH5AppSdkClient } from '@sdkwork/aiot-h5-core';
import {
  DEFAULT_DEVICE_LIST_PAGE_SIZE,
  listDevicePage,
} from '@sdkwork/aiot-app-core';

interface DeviceRow {
  deviceId: string;
  displayName: string;
  status: string;
}

export function MobileDevicePage() {
  const [devices, setDevices] = useState<DeviceRow[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [lastError, setLastError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);

  const loadPage = useCallback(async (targetPage: number) => {
    setIsLoading(true);
    setLastError(null);
    try {
      const result = await listDevicePage(getAiotH5AppSdkClient(), {
        page: targetPage,
        pageSize: DEFAULT_DEVICE_LIST_PAGE_SIZE,
      });
      setDevices(
        result.items.map((device) => ({
          deviceId: String(device.deviceId ?? device.id ?? ''),
          displayName: String(device.displayName ?? device.deviceId ?? device.id ?? ''),
          status: String(device.status ?? ''),
        })),
      );
      setPage(result.page);
      setHasMore(result.hasMore);
    } catch (error) {
      setDevices([]);
      setLastError(error instanceof Error ? error.message : '设备列表加载失败');
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadPage(1);
  }, [loadPage]);

  return (
    <div className="space-y-3 p-4">
      <h1 className="text-xl font-semibold">设备</h1>
      {isLoading ? <p className="text-sm text-zinc-500">加载中...</p> : null}
      {lastError ? <p className="text-sm text-red-600">{lastError}</p> : null}
      {devices.map((device) => (
        <article className="rounded-2xl border border-zinc-200 bg-white p-4 shadow-sm" key={device.deviceId}>
          <h2 className="font-medium">{device.displayName}</h2>
          <p className="mt-1 text-sm text-zinc-500">{device.status}</p>
        </article>
      ))}
      {!isLoading && devices.length === 0 && !lastError ? (
        <p className="text-sm text-zinc-500">暂无设备</p>
      ) : null}
      <div className="flex items-center gap-3 pt-2">
        <button
          className="rounded-full border border-zinc-300 px-4 py-2 text-sm disabled:opacity-40"
          disabled={isLoading || page <= 1}
          onClick={() => void loadPage(page - 1)}
          type="button"
        >
          上一页
        </button>
        <span className="text-sm text-zinc-500">第 {page} 页</span>
        <button
          className="rounded-full border border-zinc-300 px-4 py-2 text-sm disabled:opacity-40"
          disabled={isLoading || !hasMore}
          onClick={() => void loadPage(page + 1)}
          type="button"
        >
          下一页
        </button>
      </div>
    </div>
  );
}
