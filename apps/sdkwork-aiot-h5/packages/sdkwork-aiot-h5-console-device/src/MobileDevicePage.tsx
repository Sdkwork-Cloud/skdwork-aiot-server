import { useEffect, useState } from 'react';
import { getAiotH5AppSdkClient } from '@sdkwork/aiot-h5-core';

export function MobileDevicePage() {
  const [devices, setDevices] = useState<Array<{ deviceId: string; displayName: string; status: string }>>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [lastError, setLastError] = useState<string | null>(null);

  useEffect(() => {
    setIsLoading(true);
    setLastError(null);
    void getAiotH5AppSdkClient()
      .iot.devices.list()
      .then((page) => {
        setDevices(
          Array.isArray(page.items)
            ? page.items.map((device) => ({
                deviceId: String(device.deviceId ?? device.id ?? ''),
                displayName: String(device.displayName ?? device.deviceId ?? device.id ?? ''),
                status: String(device.status ?? ''),
              }))
            : [],
        );
      })
      .catch((error) => {
        setDevices([]);
        setLastError(error instanceof Error ? error.message : '设备列表加载失败');
      })
      .finally(() => setIsLoading(false));
  }, []);

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
    </div>
  );
}
