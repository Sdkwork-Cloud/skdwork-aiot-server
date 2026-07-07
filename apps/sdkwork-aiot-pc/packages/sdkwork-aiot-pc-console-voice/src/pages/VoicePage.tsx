import { useCallback, useEffect, useMemo, useState } from 'react';
import { Mic, MicOff, MessageSquare, Volume2 } from 'lucide-react';
import { Button, LoadingBlock, StatusNotice } from '@sdkwork/ui-pc-react';

import { createVoiceWorkspaceManifest } from '../voice';
import {
  createSdkworkVoiceService,
  type SdkworkVoiceCatalog,
  type SdkworkVoiceServicePort,
} from '../voice-service';

export interface SdkworkVoicePageProps {
  onNavigate?: (route: string) => void;
  service?: SdkworkVoiceServicePort;
}

export function SdkworkVoicePage({ service: serviceProp }: SdkworkVoicePageProps) {
  const service = useMemo(() => serviceProp ?? createSdkworkVoiceService(), [serviceProp]);
  const [catalog, setCatalog] = useState<SdkworkVoiceCatalog | null>(null);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isListening, setIsListening] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const [isSpeaking, setIsSpeaking] = useState(false);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      setCatalog(await service.getCatalog());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : '加载语音设备失败');
    } finally {
      setIsLoading(false);
    }
  }, [service]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleDialogueTurn = async () => {
    if (!draft.trim() || isSending) {
      return;
    }

    setIsSending(true);
    setError(null);
    try {
      await service.runDialogueTurn(draft.trim());
      setDraft('');
      const nextCatalog = await service.getCatalog();
      setCatalog(nextCatalog);
      setIsSpeaking(nextCatalog.isSpeaking);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : '语音对话失败');
    } finally {
      setIsSending(false);
    }
  };

  const handleToggleListen = async () => {
    if (isListening) {
      service.stopListening();
      setIsListening(false);
      return;
    }

    setIsListening(true);
    setError(null);
    try {
      await service.startListening((text, isFinal) => {
        setDraft(text);
        if (isFinal) {
          setIsListening(false);
        }
      }, {
        autoRunDialogue: true,
        onDialogueComplete: () => {
          setIsListening(false);
          void service.getCatalog().then((next) => {
            setCatalog(next);
            setIsSpeaking(next.isSpeaking);
          });
        },
        onDialogueError: (cause) => {
          setIsListening(false);
          setError(cause.message);
        },
      });
    } catch (cause) {
      setIsListening(false);
      setError(cause instanceof Error ? cause.message : '语音识别启动失败');
    }
  };

  if (isLoading && !catalog) {
    return <LoadingBlock label="加载语音设备..." />;
  }

  return (
    <div className="h-full overflow-y-auto px-4 py-4 sm:px-5 sm:py-5">
      <div className="mx-auto max-w-5xl space-y-5">
        <section className="rounded-[2rem] bg-[linear-gradient(135deg,#0f172a,#1e293b)] px-6 py-7 text-white shadow-[var(--sdk-shadow-lg)]">
          <div className="inline-flex items-center gap-2 rounded-full bg-white/10 px-3 py-1 text-[0.7rem] font-semibold uppercase tracking-[0.18em] text-white/72">
            <Volume2 className="h-3.5 w-3.5" />
            Voice Dialogue
          </div>
          <h1 className="mt-4 text-4xl font-semibold tracking-tight">智能语音对话</h1>
          <p className="mt-3 max-w-2xl text-sm leading-7 text-white/72">
            sdkwork-agents 生成回复，sdkwork-voice 负责 STT/TTS；在线设备优先通过 speak 命令播报。
          </p>
          <div className="mt-4 flex flex-wrap gap-2 text-xs">
            <span className={`rounded-full px-3 py-1 ${catalog?.agentsConfigured ? 'bg-emerald-500/20 text-emerald-100' : 'bg-white/10 text-white/60'}`}>
              Agents {catalog?.agentsConfigured ? '已连接' : '未配置'}
            </span>
            <span className={`rounded-full px-3 py-1 ${catalog?.voiceConfigured ? 'bg-cyan-500/20 text-cyan-100' : 'bg-white/10 text-white/60'}`}>
              Voice {catalog?.voiceConfigured ? '已连接' : '未配置'}
            </span>
          </div>
        </section>

        {error ? <StatusNotice tone="danger">{error}</StatusNotice> : null}

        <section className="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)]">
          <div className="rounded-3xl border border-zinc-200 bg-white p-5 shadow-sm">
            <h2 className="text-lg font-semibold text-zinc-900">语音设备</h2>
            <div className="mt-4 space-y-3">
              {(catalog?.devices ?? []).map((device) => (
                <button
                  className={`w-full rounded-2xl border px-4 py-3 text-left transition ${
                    catalog?.selectedDeviceId === device.deviceId
                      ? 'border-cyan-500 bg-cyan-50'
                      : 'border-zinc-200 hover:border-zinc-300'
                  }`}
                  key={device.deviceId}
                  onClick={() => {
                    service.selectDevice(device.deviceId);
                    void refresh();
                  }}
                  type="button"
                >
                  <div className="font-medium text-zinc-900">{device.displayName}</div>
                  <div className="mt-1 text-xs text-zinc-500">
                    {device.online ? '在线' : '离线'} · {device.chipFamily ?? device.productId ?? 'voice-device'}
                  </div>
                </button>
              ))}
              {(catalog?.devices.length ?? 0) === 0 ? (
                <p className="text-sm text-zinc-500">暂无可用语音设备，将使用云端 Voice TTS 或浏览器本地播报。</p>
              ) : null}
            </div>
          </div>

          <div className="rounded-3xl border border-zinc-200 bg-white p-5 shadow-sm">
            <h2 className="text-lg font-semibold text-zinc-900">对话输入</h2>
            <textarea
              className="mt-4 min-h-40 w-full rounded-2xl border border-zinc-200 px-4 py-3 text-sm outline-none focus:border-cyan-500"
              disabled={isSending}
              onChange={(event) => setDraft(event.target.value)}
              placeholder="输入文字或使用麦克风说话，然后发起完整语音对话..."
              value={draft}
            />
            <div className="mt-4 flex flex-wrap gap-3">
              <Button disabled={isSending} onClick={() => void handleToggleListen()} type="button" variant="outline">
                {isListening ? <MicOff className="mr-2 h-4 w-4" /> : <Mic className="mr-2 h-4 w-4" />}
                {isListening ? '停止聆听' : '开始聆听'}
              </Button>
              <Button disabled={isSending || isSpeaking || !draft.trim()} onClick={() => void handleDialogueTurn()} type="button">
                <MessageSquare className="mr-2 h-4 w-4" />
                {isSending ? '对话中...' : isSpeaking ? '播报中...' : '发起语音对话'}
              </Button>
            </div>
            {catalog?.transcript ? (
              <p className="mt-4 text-sm text-zinc-500">识别结果：{catalog.transcript}</p>
            ) : null}
            {catalog?.lastAssistantReply ? (
              <div className="mt-4 rounded-2xl bg-zinc-50 px-4 py-3 text-sm text-zinc-700">
                <div className="text-xs font-semibold uppercase tracking-[0.12em] text-zinc-400">Assistant</div>
                <p className="mt-2 leading-6">{catalog.lastAssistantReply}</p>
              </div>
            ) : null}
          </div>
        </section>
      </div>
    </div>
  );
}

export const sdkworkVoiceWorkspaceManifest = createVoiceWorkspaceManifest();
