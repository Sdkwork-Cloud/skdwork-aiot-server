import { readOptionalBearerToken, readProcessEnv } from '@sdkwork/aiot-app-core';

/** Session tokens resolved from appbase IAM runtime storage — never from VITE_* env. */
export interface H5RuntimeSession {
  accessToken?: string;
  authToken?: string;
}

const IAM_AUTH_TOKEN_STORAGE_KEYS = [
  'sdkwork.core.pc-react.auth-token',
  'sdkwork_token',
] as const;

const IAM_ACCESS_TOKEN_STORAGE_KEYS = ['core.pc-react.access-token'] as const;

function readBrowserStorageToken(keys: readonly string[]): string | undefined {
  if (typeof window === 'undefined') {
    return undefined;
  }

  try {
    for (const key of keys) {
      const value = window.localStorage.getItem(key);
      if (value?.trim()) {
        return value.trim();
      }
    }
  } catch {
    return undefined;
  }

  return undefined;
}

/**
 * Reads IAM session tokens from centralized browser storage populated by appbase login.
 * Server-side bootstrap may supply `SDKWORK_AUTH_TOKEN` / `SDKWORK_ACCESS_TOKEN` via process env only.
 */
export function readH5RuntimeSession(): H5RuntimeSession {
  return {
    authToken: readOptionalBearerToken(
      readBrowserStorageToken(IAM_AUTH_TOKEN_STORAGE_KEYS)
        ?? readProcessEnv('SDKWORK_AUTH_TOKEN'),
    ),
    accessToken: readOptionalBearerToken(
      readBrowserStorageToken(IAM_ACCESS_TOKEN_STORAGE_KEYS)
        ?? readProcessEnv('SDKWORK_ACCESS_TOKEN'),
    ),
  };
}
