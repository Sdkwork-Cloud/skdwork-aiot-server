import { createTokenManager, type AuthTokenManager } from '@sdkwork/aiot-app-sdk';

import { readH5RuntimeSession } from './h5RuntimeSession';

let h5TokenManager: AuthTokenManager | null = null;

export function syncH5TokenManagerFromRuntimeSession(manager: AuthTokenManager): void {
  const session = readH5RuntimeSession();
  if (session.authToken || session.accessToken) {
    manager.setTokens({
      ...(session.authToken ? { authToken: session.authToken } : {}),
      ...(session.accessToken ? { accessToken: session.accessToken } : {}),
    });
    return;
  }

  manager.clearTokens();
}

export function getAiotH5TokenManager(): AuthTokenManager {
  if (!h5TokenManager) {
    h5TokenManager = createTokenManager();
  }
  syncH5TokenManagerFromRuntimeSession(h5TokenManager);
  return h5TokenManager;
}

export function resetAiotH5TokenManager(): void {
  h5TokenManager = null;
}
