import type { ReactNode } from 'react';
import { useEffect, useState } from 'react';

import { readH5RuntimeSession } from '../sdk/h5RuntimeSession';
import { syncH5TokenManagerFromRuntimeSession, getAiotH5TokenManager } from '../sdk/h5TokenManager';
import { readImportMetaEnvWithDefault } from '@sdkwork/aiot-app-core';

function hasAuthenticatedSession(): boolean {
  const session = readH5RuntimeSession();
  return Boolean(session.authToken && session.accessToken);
}

function resolveLoginUrl(): string {
  const gatewayBase = readImportMetaEnvWithDefault(
    'VITE_SDKWORK_AIOT_PLATFORM_API_GATEWAY_HTTP_URL',
    'http://127.0.0.1:3900',
  );
  const redirect = encodeURIComponent(window.location.pathname + window.location.search);
  return `${gatewayBase.replace(/\/$/, '')}/auth/login?redirect=${redirect}`;
}

export interface AiotH5AuthGateProps {
  children: ReactNode;
}

/**
 * Protects H5 console routes until appbase IAM has persisted dual-token session state.
 */
export function AiotH5AuthGate({ children }: AiotH5AuthGateProps) {
  const [authenticated, setAuthenticated] = useState(hasAuthenticatedSession());

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }

    const syncSession = () => {
      syncH5TokenManagerFromRuntimeSession(getAiotH5TokenManager());
      setAuthenticated(hasAuthenticatedSession());
    };

    syncSession();
    window.addEventListener('storage', syncSession);
    return () => window.removeEventListener('storage', syncSession);
  }, []);

  if (authenticated) {
    return children;
  }

  return (
    <div className="flex min-h-screen flex-col items-center justify-center bg-zinc-100 px-6 text-center">
      <h1 className="text-xl font-semibold text-zinc-900">需要登录</h1>
      <p className="mt-3 max-w-sm text-sm text-zinc-600">
        请通过 SDKWork IAM 完成登录后再访问 AIoT 控制台。会话令牌由 appbase 登录流程写入浏览器存储，不会从公开环境变量读取。
      </p>
      <a
        className="mt-6 inline-flex rounded-full bg-cyan-700 px-5 py-2.5 text-sm font-medium text-white"
        href={resolveLoginUrl()}
      >
        前往登录
      </a>
    </div>
  );
}
