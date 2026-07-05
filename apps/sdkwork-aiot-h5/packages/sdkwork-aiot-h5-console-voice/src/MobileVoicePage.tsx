import { useCallback, useEffect, useMemo, useState } from 'react';
import { createAiotH5VoiceDialogueService } from '@sdkwork/aiot-h5-core';
import type { AiotVoiceDialogueCatalog } from '@sdkwork/aiot-app-core';

export function MobileVoicePage() {
  const service = useMemo(() => createAiotH5VoiceDialogueService(), []);
  const [catalog, setCatalog] = useState<AiotVoiceDialogueCatalog | null>(null);
  const [draft, setDraft] = useState('');
  const [isListening, setIsListening] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setCatalog(await service.getCatalog());
    } catch (error) {
      setLastError(error instanceof Error ? error.message : '语音设备加载失败');
    }
  }, [service]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleDialogue = async () => {
    if (!draft.trim() || isSending) {
      return;
    }

    setIsSending(true);
    setLastError(null);
    try {
      await service.runDialogueTurn(draft.trim());
      setDraft('');
      await refresh();
    } catch (error) {
      setLastError(error instanceof Error ? error.message : '语音对话失败');
    } finally {
      setIsSending(false);
    }
  };

  const handleListen = async () => {
    if (isListening) {
      service.stopListening();
      setIsListening(false);
      return;
    }

    setIsListening(true);
    setLastError(null);
    try {
      await service.startListening((text, isFinal) => {
        setDraft(text);
        if (isFinal) {
          setIsListening(false);
          void service.getCatalog().then(setCatalog);
        }
      }, { autoRunDialogue: true });
    } catch (error) {
      setIsListening(false);
      setLastError(error instanceof Error ? error.message : '语音识别启动失败');
    }
  };

  return (
    <div className="space-y-4 p-4">
      <h1 className="text-xl font-semibold">语音对话</h1>
      <div className="flex flex-wrap gap-2 text-xs">
        <span className={`rounded-full px-3 py-1 ${catalog?.agentsConfigured ? 'bg-emerald-100 text-emerald-700' : 'bg-zinc-100 text-zinc-500'}`}>
          Agents {catalog?.agentsConfigured ? '已连接' : '未配置'}
        </span>
        <span className={`rounded-full px-3 py-1 ${catalog?.voiceConfigured ? 'bg-cyan-100 text-cyan-700' : 'bg-zinc-100 text-zinc-500'}`}>
          Voice {catalog?.voiceConfigured ? '已连接' : '未配置'}
        </span>
      </div>
      {lastError ? <p className="text-sm text-red-600">{lastError}</p> : null}
      <select
        className="w-full rounded-2xl border border-zinc-200 px-3 py-2 text-sm"
        onChange={(event) => {
          service.selectDevice(event.target.value || null);
          void refresh();
        }}
        value={catalog?.selectedDeviceId ?? ''}
      >
        {(catalog?.devices ?? []).map((device) => (
          <option key={device.deviceId} value={device.deviceId}>{device.displayName}</option>
        ))}
      </select>
      <textarea
        className="min-h-28 w-full rounded-2xl border border-zinc-200 px-3 py-2 text-sm"
        disabled={isSending}
        onChange={(event) => setDraft(event.target.value)}
        placeholder="输入或说出指令..."
        value={draft}
      />
      <div className="flex gap-2">
        <button
          className="flex-1 rounded-2xl border border-zinc-300 px-4 py-3 text-sm"
          disabled={isSending}
          onClick={() => void handleListen()}
          type="button"
        >
          {isListening ? '停止聆听' : '开始聆听'}
        </button>
        <button
          className="flex-1 rounded-2xl bg-cyan-600 px-4 py-3 text-sm font-medium text-white disabled:opacity-60"
          disabled={isSending || !draft.trim()}
          onClick={() => void handleDialogue()}
          type="button"
        >
          {isSending ? '对话中...' : '发起对话'}
        </button>
      </div>
      {catalog?.lastAssistantReply ? (
        <div className="rounded-2xl bg-zinc-50 px-3 py-2 text-sm text-zinc-700">{catalog.lastAssistantReply}</div>
      ) : null}
    </div>
  );
}
