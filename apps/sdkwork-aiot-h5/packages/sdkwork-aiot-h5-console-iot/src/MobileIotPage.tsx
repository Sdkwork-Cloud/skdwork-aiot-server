import { useEffect, useState } from 'react';
import { getAiotH5AppSdkClient } from '@sdkwork/aiot-h5-core';

export function MobileIotPage() {
  const [nodes, setNodes] = useState<Array<{ id: string; name: string; status: string }>>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [lastError, setLastError] = useState<string | null>(null);

  useEffect(() => {
    setIsLoading(true);
    setLastError(null);
    void getAiotH5AppSdkClient()
      .iot.devicesList()
      .then((response) => {
        setNodes(
          Array.isArray(response.data)
            ? response.data.map((device) => ({
                id: device.deviceId || device.id,
                name: device.displayName || device.deviceId || device.id,
                status: device.status,
              }))
            : [],
        );
      })
      .catch((error) => {
        setNodes([]);
        setLastError(error instanceof Error ? error.message : 'IoT 舰队加载失败');
      })
      .finally(() => setIsLoading(false));
  }, []);

  return (
    <div className="space-y-3 p-4">
      <h1 className="text-xl font-semibold">IoT 舰队</h1>
      {isLoading ? <p className="text-sm text-zinc-500">加载中...</p> : null}
      {lastError ? <p className="text-sm text-red-600">{lastError}</p> : null}
      {nodes.map((node) => (
        <article className="rounded-2xl border border-zinc-200 bg-white p-4 shadow-sm" key={node.id}>
          <h2 className="font-medium">{node.name}</h2>
          <p className="mt-1 text-sm text-zinc-500">{node.status}</p>
        </article>
      ))}
      {!isLoading && nodes.length === 0 && !lastError ? (
        <p className="text-sm text-zinc-500">暂无 IoT 节点</p>
      ) : null}
    </div>
  );
}
