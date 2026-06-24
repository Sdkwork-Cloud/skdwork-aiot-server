import { BrowserRouter, Navigate, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { MobileAgentPage } from '@sdkwork/aiot-h5-console-agent';
import { MobileDevicePage } from '@sdkwork/aiot-h5-console-device';
import { MobileIotPage } from '@sdkwork/aiot-h5-console-iot';
import { MobileVoicePage } from '@sdkwork/aiot-h5-console-voice';
import { AiotH5AuthGate, initAiotH5AppSdkClient } from '@sdkwork/aiot-h5-core';

initAiotH5AppSdkClient();

const NAV = [
  { path: '/devices', label: '设备' },
  { path: '/iot', label: 'IoT' },
  { path: '/voice', label: '语音' },
  { path: '/agent', label: '智能体' },
] as const;

function MobileShell() {
  const location = useLocation();
  const navigate = useNavigate();

  return (
    <div className="flex min-h-screen flex-col bg-zinc-100">
      <main className="flex-1 pb-20">
        <Routes>
          <Route path="/" element={<Navigate replace to="/devices" />} />
          <Route path="/devices" element={<MobileDevicePage />} />
          <Route path="/iot" element={<MobileIotPage />} />
          <Route path="/voice" element={<MobileVoicePage />} />
          <Route path="/agent" element={<MobileAgentPage />} />
        </Routes>
      </main>
      <nav className="fixed inset-x-0 bottom-0 grid grid-cols-4 border-t border-zinc-200 bg-white">
        {NAV.map((item) => {
          const active = location.pathname === item.path;
          return (
            <button
              className={`py-3 text-xs font-medium ${active ? 'text-cyan-700' : 'text-zinc-500'}`}
              key={item.path}
              onClick={() => navigate(item.path)}
              type="button"
            >
              {item.label}
            </button>
          );
        })}
      </nav>
    </div>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <AiotH5AuthGate>
        <MobileShell />
      </AiotH5AuthGate>
    </BrowserRouter>
  );
}
