import { afterEach, describe, expect, it } from 'vitest';

import {
  isAgentsAppSdkConfigured,
  resolveAiotAppApiBaseUrl,
  resolveAiotEdgeIngressWebSocketBaseUrl,
} from '../src/sdk/sdkBaseUrls';

const BASE_URL_ENV_KEY = 'SDKWORK_API_BASE_URL';

const originalBaseUrlEnv = process.env[BASE_URL_ENV_KEY];

afterEach(() => {
  if (originalBaseUrlEnv === undefined) {
    delete process.env[BASE_URL_ENV_KEY];
  } else {
    process.env[BASE_URL_ENV_KEY] = originalBaseUrlEnv;
  }
});

describe('sdkwork-aiot-pc-core sdkBaseUrls', () => {
  it('resolves base urls from the unified SDKWORK_API_BASE_URL key and strips sdk-owned paths', () => {
    process.env[BASE_URL_ENV_KEY] = 'http://api-test.example.com/app/v3/api/iot';

    expect(resolveAiotAppApiBaseUrl()).toBe('http://api-test.example.com');
  });

  it('reports sibling sdk configured when the shared base-url key is set', () => {
    process.env[BASE_URL_ENV_KEY] = 'http://api-test.example.com';

    expect(isAgentsAppSdkConfigured()).toBe(true);
  });

  it('derives edge websocket url from edge ingress http url when websocket env is unset', () => {
    delete process.env.VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_HTTP_URL;
    delete process.env.VITE_SDKWORK_AIOT_EDGE_DEVICE_INGRESS_WEBSOCKET_URL;

    expect(resolveAiotEdgeIngressWebSocketBaseUrl()).toBe('ws://127.0.0.1:18080');
  });
});
