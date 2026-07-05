import { useEffect, useState } from 'react';
import { Bot, Cpu, Mic, RadioTower } from 'lucide-react';
import { SdkworkAgentPage } from '@sdkwork/aiot-pc-console-agent';
import { SdkworkDevicePage } from '@sdkwork/aiot-pc-console-device';
import { SdkworkIotPage } from '@sdkwork/aiot-pc-console-iot';
import { SdkworkVoicePage } from '@sdkwork/aiot-pc-console-voice';
import { initAiotAppSdkClient, initAiotBackendSdkClient, initAgentsAppSdkClient, initDriveAppSdkClient, initVoiceAppSdkClient } from '@sdkwork/aiot-pc-core';

type AiotRoute = '/devices' | '/iot' | '/voice' | '/agent';

const NAV_ITEMS: Array<{ icon: typeof Cpu; id: AiotRoute; label: string }> = [
  { icon: Cpu, id: '/devices', label: '设备' },
  { icon: RadioTower, id: '/iot', label: 'IoT 舰队' },
  { icon: Mic, id: '/voice', label: '语音对话' },
  { icon: Bot, id: '/agent', label: '智能体' },
];

function resolveRouteIntent(route: string): AiotRoute {
  if (route.startsWith('/iot')) {
    return '/iot';
  }
  if (route.startsWith('/voice')) {
    return '/voice';
  }
  if (route.startsWith('/agent')) {
    return '/agent';
  }
  return '/devices';
}

export function App() {
  const [route, setRoute] = useState<AiotRoute>('/devices');

  useEffect(() => {
    initAiotAppSdkClient();
    initDriveAppSdkClient();
    initAiotBackendSdkClient();
    initAgentsAppSdkClient();
    initVoiceAppSdkClient();
  }, []);

  const handleNavigate = (targetRoute: string) => {
    setRoute(resolveRouteIntent(targetRoute));
  };

  return (
    <div className="flex min-h-screen bg-zinc-50">
      <aside className="hidden w-64 shrink-0 border-r border-zinc-200 bg-white px-4 py-6 md:block">
        <div className="mb-8 px-2">
          <div className="text-xs font-semibold uppercase tracking-[0.18em] text-cyan-600">SDKWork AIoT</div>
          <h1 className="mt-2 text-2xl font-semibold text-zinc-900">应用控制台</h1>
        </div>
        <nav className="space-y-2">
          {NAV_ITEMS.map((item) => {
            const Icon = item.icon;
            const active = route === item.id;
            return (
              <button
                className={`flex w-full items-center gap-3 rounded-2xl px-3 py-3 text-sm font-medium transition ${
                  active ? 'bg-zinc-900 text-white' : 'text-zinc-600 hover:bg-zinc-100'
                }`}
                key={item.id}
                onClick={() => setRoute(item.id)}
                type="button"
              >
                <Icon className="h-4 w-4" />
                {item.label}
              </button>
            );
          })}
        </nav>
      </aside>

      <main className="flex-1">
        <div className="border-b border-zinc-200 bg-white px-4 py-3 md:hidden">
          <div className="flex gap-2 overflow-x-auto">
            {NAV_ITEMS.map((item) => (
              <button
                className={`whitespace-nowrap rounded-full px-3 py-1.5 text-xs font-medium ${
                  route === item.id ? 'bg-zinc-900 text-white' : 'bg-zinc-100 text-zinc-600'
                }`}
                key={item.id}
                onClick={() => setRoute(item.id)}
                type="button"
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>

        {route === '/devices' ? <SdkworkDevicePage onNavigate={handleNavigate} /> : null}
        {route === '/iot' ? <SdkworkIotPage onNavigate={handleNavigate} /> : null}
        {route === '/voice' ? <SdkworkVoicePage /> : null}
        {route === '/agent' ? <SdkworkAgentPage /> : null}
      </main>
    </div>
  );
}
