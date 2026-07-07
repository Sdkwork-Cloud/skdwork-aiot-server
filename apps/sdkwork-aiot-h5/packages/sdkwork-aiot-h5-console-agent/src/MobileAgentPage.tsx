import { useEffect, useRef, useState } from 'react';
import {
  createAiotH5AgentService,
  getAiotH5AppSdkClient,
  listDevicePage,
  readDeviceId,
} from '@sdkwork/aiot-h5-core';
import type { AiotAgentService } from '@sdkwork/aiot-app-core';

export function MobileAgentPage() {
  const agentServiceRef = useRef<AiotAgentService | null>(null);
  const [devices, setDevices] = useState<Array<{ deviceId: string; displayName: string }>>([]);
  const [devicePage, setDevicePage] = useState(1);
  const [deviceHasMore, setDeviceHasMore] = useState(false);
  const [isLoadingDevices, setIsLoadingDevices] = useState(false);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Array<{ id: string; role: string; content: string }>>([]);
  const [draft, setDraft] = useState('');
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [isSending, setIsSending] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  useEffect(() => {
    setIsLoadingDevices(true);
    void listDevicePage(getAiotH5AppSdkClient(), { page: devicePage })
      .then((result) => {
        const mapped = result.items.map((device) => ({
          deviceId: readDeviceId(device),
          displayName: String(device.displayName ?? readDeviceId(device)),
        }));
        setDevices((current) => (devicePage === 1 ? mapped : [...current, ...mapped]));
        setDeviceHasMore(result.hasMore);
      })
      .catch((error) => {
        setLastError(error instanceof Error ? error.message : '设备列表加载失败');
      })
      .finally(() => {
        setIsLoadingDevices(false);
      });
  }, [devicePage]);

  const ensureSession = (deviceId: string) => {
    if (!agentServiceRef.current) {
      agentServiceRef.current = createAiotH5AgentService();
    }
    const agentService = agentServiceRef.current;
    const session = agentService.createSession(deviceId, '移动端智能体');
    setSessionId(session.id);
    setMessages(agentService.getMessages(session.id));
    return session;
  };

  const handleSend = async () => {
    if (!draft.trim() || isSending) {
      return;
    }

    if (!selectedDeviceId) {
      setLastError('请先选择设备');
      return;
    }

    const session = sessionId ? { id: sessionId } : ensureSession(selectedDeviceId);
    const agentService = agentServiceRef.current;
    if (!agentService) {
      return;
    }

    setIsSending(true);
    setLastError(null);
    try {
      await agentService.sendMessage({
        deviceId: selectedDeviceId,
        sessionId: session.id,
        text: draft.trim(),
      });
      setMessages(agentService.getMessages(session.id));
      setDraft('');
    } catch (error) {
      setLastError(error instanceof Error ? error.message : '智能体消息发送失败');
    } finally {
      setIsSending(false);
    }
  };

  return (
    <div className="flex h-full flex-col p-4">
      <h1 className="text-xl font-semibold">智能体</h1>
      {lastError ? <p className="mt-2 text-sm text-red-600">{lastError}</p> : null}
      <div className="mt-3 space-y-2">
        {devices.map((device) => (
          <button
            className={`w-full rounded-2xl border px-3 py-2 text-left text-sm ${
              selectedDeviceId === device.deviceId
                ? 'border-indigo-500 bg-indigo-50 text-indigo-700'
                : 'border-zinc-200 text-zinc-700'
            }`}
            key={device.deviceId}
            onClick={() => {
              setSelectedDeviceId(device.deviceId);
              setMessages([]);
              setSessionId(null);
            }}
            type="button"
          >
            {device.displayName}
          </button>
        ))}
        {devices.length === 0 && !isLoadingDevices ? (
          <p className="text-sm text-zinc-500">暂无可用设备</p>
        ) : null}
        {deviceHasMore ? (
          <button
            className="w-full rounded-2xl border border-zinc-200 px-3 py-2 text-sm text-zinc-700"
            disabled={isLoadingDevices}
            onClick={() => setDevicePage((current) => current + 1)}
            type="button"
          >
            {isLoadingDevices ? '加载中...' : '加载更多设备'}
          </button>
        ) : null}
      </div>
      <div className="mt-4 flex-1 space-y-3 overflow-y-auto">
        {messages.map((message) => (
          <div
            className={`max-w-[85%] rounded-2xl px-3 py-2 text-sm ${
              message.role === 'user' ? 'ml-auto bg-indigo-600 text-white' : 'bg-white text-zinc-800'
            }`}
            key={message.id}
          >
            {message.content}
          </div>
        ))}
      </div>
      <div className="mt-4 flex gap-2">
        <input
          className="flex-1 rounded-2xl border border-zinc-200 px-3 py-2 text-sm"
          disabled={isSending}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="向智能体提问..."
          value={draft}
        />
        <button
          className="rounded-2xl bg-indigo-600 px-4 py-2 text-sm text-white disabled:opacity-60"
          disabled={isSending}
          onClick={() => void handleSend()}
          type="button"
        >
          {isSending ? '发送中...' : '发送'}
        </button>
      </div>
    </div>
  );
}
