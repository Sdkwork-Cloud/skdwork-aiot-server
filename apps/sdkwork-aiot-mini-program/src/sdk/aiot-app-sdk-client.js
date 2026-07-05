/**
 * WeChat mini program transport adapter for the @sdkwork/aiot-app-sdk contract.
 * Business modules must consume this client instead of assembling raw wx.request calls.
 */

const STORAGE_KEYS = {
  appApiBaseUrl: 'SDKWORK_AIOT_APP_API_BASE_URL',
  authToken: 'SDKWORK_AUTH_TOKEN',
  accessToken: 'SDKWORK_ACCESS_TOKEN',
};

const DEFAULT_APP_API_BASE_URL = 'http://127.0.0.1:18082';
const DEFAULT_LIST_PAGE_SIZE = 20;
const COMMAND_POLL_INTERVAL_MS = 400;
const COMMAND_POLL_MAX_ATTEMPTS = 12;

let cachedClient = null;

function readStorage(key) {
  const value = wx.getStorageSync(key);
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

function createAiotAppSdkClientConfig() {
  return {
    baseUrl: readStorage(STORAGE_KEYS.appApiBaseUrl) || DEFAULT_APP_API_BASE_URL,
    authToken: readStorage(STORAGE_KEYS.authToken),
    accessToken: readStorage(STORAGE_KEYS.accessToken),
    platform: 'mini-program',
  };
}

function sleep(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function readRecord(value) {
  return value && typeof value === 'object' ? value : {};
}

function readString(value, fallback = '') {
  return typeof value === 'string' && value.trim() ? value.trim() : fallback;
}

function unwrapListPayload(data) {
  const record = readRecord(data);
  const items = Array.isArray(record.items)
    ? record.items
    : Array.isArray(data)
      ? data
      : [];
  const pageInfo = readRecord(record.pageInfo);
  return {
    items,
    pageInfo: {
      hasMore: Boolean(pageInfo.hasMore),
      page: typeof pageInfo.page === 'number' ? pageInfo.page : 1,
      pageSize: typeof pageInfo.pageSize === 'number' ? pageInfo.pageSize : DEFAULT_LIST_PAGE_SIZE,
      total: typeof pageInfo.total === 'number' ? pageInfo.total : undefined,
    },
  };
}

function unwrapCommandAcceptance(data) {
  const record = readRecord(data);
  return {
    commandId: readString(record.resourceId, readString(record.commandId)),
    status: readString(record.status, 'accepted'),
    resultPayload: record.resultPayload,
  };
}

function request(method, path, options = {}) {
  const config = createAiotAppSdkClientConfig();
  const headers = {
    'Content-Type': 'application/json',
  };

  if (config.authToken) {
    headers.Authorization = `Bearer ${config.authToken}`;
  }
  if (config.accessToken) {
    headers['Access-Token'] = config.accessToken;
  }
  if (options.idempotencyKey) {
    headers['Idempotency-Key'] = options.idempotencyKey;
  }

  return new Promise((resolve, reject) => {
    wx.request({
      url: `${config.baseUrl}/app/v3/api${path}`,
      method,
      data: options.data,
      header: headers,
      success(response) {
        if (response.statusCode >= 200 && response.statusCode < 300) {
          const body = readRecord(response.data);
          if (typeof body.code === 'number' && body.code !== 0) {
            reject(new Error(`api.business.${body.code}`));
            return;
          }
          resolve({
            code: typeof body.code === 'number' ? body.code : 0,
            data: body.data ?? body,
          });
          return;
        }

        reject(new Error(`api.http.${response.statusCode}`));
      },
      fail(error) {
        reject(error instanceof Error ? error : new Error(String(error?.errMsg || 'request failed')));
      },
    });
  });
}

async function pollCommandResult(deviceId, commandId) {
  for (let attempt = 0; attempt < COMMAND_POLL_MAX_ATTEMPTS; attempt += 1) {
    let page = 1;
    while (true) {
      const response = await request('GET', `/iot/devices/${deviceId}/events`, {
        data: {
          page,
          page_size: DEFAULT_LIST_PAGE_SIZE,
          q: commandId,
        },
      });
      const list = unwrapListPayload(response.data);
      const match = list.items.find((event) => {
        const payload = readRecord(event.payload);
        const correlationId = readString(payload.correlationId, readString(payload.commandId));
        return correlationId === commandId;
      });
      if (match) {
        const payload = readRecord(match.payload);
        return {
          commandId,
          resultPayload: payload.result ?? payload,
          status: readString(payload.status, 'completed'),
        };
      }
      if (!list.pageInfo.hasMore || list.items.length === 0) {
        break;
      }
      page += 1;
    }
    await sleep(COMMAND_POLL_INTERVAL_MS);
  }
  return null;
}

function createAiotAppSdkClient() {
  return {
    iot: {
      devicesList(params = {}) {
        return request('GET', '/iot/devices', {
          data: {
            page: params.page ?? 1,
            page_size: params.pageSize ?? DEFAULT_LIST_PAGE_SIZE,
          },
        }).then((response) => ({
          code: response.code,
          data: unwrapListPayload(response.data),
        }));
      },
      devicesCommandsCreate(deviceId, body, idempotencyKey) {
        return request('POST', `/iot/devices/${deviceId}/commands`, {
          data: body,
          idempotencyKey,
        }).then((response) => ({
          code: response.code,
          data: unwrapCommandAcceptance(response.data),
        }));
      },
      pollCommandResult(deviceId, commandId) {
        return pollCommandResult(deviceId, commandId);
      },
    },
  };
}

function getAiotAppSdkClient() {
  if (!cachedClient) {
    cachedClient = createAiotAppSdkClient();
  }
  return cachedClient;
}

function resetAiotAppSdkClient() {
  cachedClient = null;
}

module.exports = {
  createAiotAppSdkClient,
  createAiotAppSdkClientConfig,
  getAiotAppSdkClient,
  resetAiotAppSdkClient,
};
