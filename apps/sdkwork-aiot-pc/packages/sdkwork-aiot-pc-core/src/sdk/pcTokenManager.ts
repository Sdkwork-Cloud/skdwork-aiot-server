import { createTokenManager, type AuthTokenManager } from '@sdkwork/aiot-app-sdk';
import { readPcReactRuntimeSession } from '@sdkwork/core-pc-react';
import { readOptionalBearerToken } from '@sdkwork/aiot-app-core';

let pcTokenManager: AuthTokenManager | null = null;

export function syncPcTokenManagerFromRuntimeSession(manager: AuthTokenManager): void {
  const session = readPcReactRuntimeSession();
  const authToken = readOptionalBearerToken(session.authToken);
  const accessToken = readOptionalBearerToken(session.accessToken);
  if (authToken || accessToken) {
    manager.setTokens({
      ...(authToken ? { authToken } : {}),
      ...(accessToken ? { accessToken } : {}),
    });
    return;
  }

  manager.clearTokens();
}

export function getAiotPcTokenManager(): AuthTokenManager {
  if (!pcTokenManager) {
    pcTokenManager = createTokenManager();
  }
  syncPcTokenManagerFromRuntimeSession(pcTokenManager);
  return pcTokenManager;
}

export function resetAiotPcTokenManager(): void {
  pcTokenManager = null;
}
