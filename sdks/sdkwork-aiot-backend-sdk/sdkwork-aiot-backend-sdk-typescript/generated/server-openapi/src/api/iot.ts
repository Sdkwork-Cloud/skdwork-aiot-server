import { backendApiPath } from './paths';
import type { ApiRequestOptions, HttpClient } from '../http/client';

import type { AiotCapabilityModelCreateRequest, AiotCapabilityModelResponse, AiotCapabilityModelUpdateRequest, AiotCredentialCreateRequest, AiotCredentialResponse, AiotDeviceCreateRequest, AiotDeviceUpdateRequest, AiotFirmwareArtifactCreateRequest, AiotFirmwareArtifactResponse, AiotFirmwareArtifactUpdateRequest, AiotFirmwareRolloutCreateRequest, AiotFirmwareRolloutResponse, AiotFirmwareRolloutUpdateRequest, AiotHardwareProfileCreateRequest, AiotHardwareProfileResponse, AiotHardwareProfileUpdateRequest, AiotProductCreateRequest, AiotProductResponse, AiotProductUpdateRequest, AiotProtocolProfileCreateRequest, AiotProtocolProfileResponse, AiotProtocolProfileUpdateRequest, AiotRuntimeCapacityPolicyResponse, AiotTwinUpdateRequest, JsonValue, SdkWorkCommandData, SdkWorkPageData } from '../types';


export class IotRuntimeCapacityApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Retrieve AIoT runtime capacity and backpressure policy */
  async retrieve(requestOptions?: ApiRequestOptions): Promise<AiotRuntimeCapacityPolicyResponse> {
    return this.client.request<AiotRuntimeCapacityPolicyResponse>(backendApiPath(`/iot/runtime/capacity`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export class IotRuntimeApi {
  public readonly capacity: IotRuntimeCapacityApi;

  constructor(client: HttpClient) {
    this.capacity = new IotRuntimeCapacityApi(client);
  }

}

export interface IotProtocolAdaptersListParams {
  page?: number;
  pageSize?: number;
}

export class IotProtocolAdaptersApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List protocol adapters */
  async list(params?: IotProtocolAdaptersListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/protocol_adapters`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface IotEventsListParams {
  page?: number;
  pageSize?: number;
}

export class IotEventsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List platform IoT events */
  async list(params?: IotEventsListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/events`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface IotFirmwareRolloutsListParams {
  page?: number;
  pageSize?: number;
}

export interface IotFirmwareRolloutsCreateParams {
  idempotencyKey?: string;
}

export class IotFirmwareRolloutsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List firmware rollouts */
  async list(params?: IotFirmwareRolloutsListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/firmware_rollouts`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Create firmware rollout */
  async create(body: AiotFirmwareRolloutCreateRequest, params?: IotFirmwareRolloutsCreateParams, requestOptions?: ApiRequestOptions): Promise<AiotFirmwareRolloutResponse> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<AiotFirmwareRolloutResponse>(backendApiPath(`/iot/firmware_rollouts`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Retrieve firmware rollout */
  async retrieve(rolloutId: string, requestOptions?: ApiRequestOptions): Promise<AiotFirmwareRolloutResponse> {
    return this.client.request<AiotFirmwareRolloutResponse>(backendApiPath(`/iot/firmware_rollouts/${serializePathParameter(rolloutId, { name: 'rolloutId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Update firmware rollout */
  async update(rolloutId: string, body?: AiotFirmwareRolloutUpdateRequest, requestOptions?: ApiRequestOptions): Promise<AiotFirmwareRolloutResponse> {
    return this.client.request<AiotFirmwareRolloutResponse>(backendApiPath(`/iot/firmware_rollouts/${serializePathParameter(rolloutId, { name: 'rolloutId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PUT' as any, ...(body !== undefined ? { body, contentType: 'application/json' } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Delete firmware rollout */
  async delete(rolloutId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/firmware_rollouts/${serializePathParameter(rolloutId, { name: 'rolloutId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface IotFirmwareArtifactsListParams {
  page?: number;
  pageSize?: number;
}

export interface IotFirmwareArtifactsCreateParams {
  idempotencyKey?: string;
}

export class IotFirmwareArtifactsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List firmware artifacts */
  async list(params?: IotFirmwareArtifactsListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/firmware_artifacts`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Create firmware artifact metadata */
  async create(body: AiotFirmwareArtifactCreateRequest, params?: IotFirmwareArtifactsCreateParams, requestOptions?: ApiRequestOptions): Promise<AiotFirmwareArtifactResponse> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<AiotFirmwareArtifactResponse>(backendApiPath(`/iot/firmware_artifacts`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Retrieve firmware artifact */
  async retrieve(artifactId: string, requestOptions?: ApiRequestOptions): Promise<AiotFirmwareArtifactResponse> {
    return this.client.request<AiotFirmwareArtifactResponse>(backendApiPath(`/iot/firmware_artifacts/${serializePathParameter(artifactId, { name: 'artifactId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Update firmware artifact metadata */
  async update(artifactId: string, body?: AiotFirmwareArtifactUpdateRequest, requestOptions?: ApiRequestOptions): Promise<AiotFirmwareArtifactResponse> {
    return this.client.request<AiotFirmwareArtifactResponse>(backendApiPath(`/iot/firmware_artifacts/${serializePathParameter(artifactId, { name: 'artifactId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PUT' as any, ...(body !== undefined ? { body, contentType: 'application/json' } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Delete firmware artifact metadata */
  async delete(artifactId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/firmware_artifacts/${serializePathParameter(artifactId, { name: 'artifactId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export class IotDevicesTwinApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Update backend device twin */
  async update(deviceId: string, body: AiotTwinUpdateRequest, requestOptions?: ApiRequestOptions): Promise<JsonValue> {
    return this.client.request<JsonValue>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/twin`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** Retrieve backend device twin */
  async retrieve(deviceId: string, requestOptions?: ApiRequestOptions): Promise<JsonValue> {
    return this.client.request<JsonValue>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/twin`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface IotDevicesCommandsListParams {
  page?: number;
  pageSize?: number;
}

export class IotDevicesCommandsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List device commands */
  async list(deviceId: string, params?: IotDevicesCommandsListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/commands`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Retrieve device command */
  async retrieve(deviceId: string, commandId: string, requestOptions?: ApiRequestOptions): Promise<Record<string, unknown>> {
    return this.client.request<Record<string, unknown>>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/commands/${serializePathParameter(commandId, { name: 'commandId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Cancel device command */
  async cancel(deviceId: string, commandId: string, requestOptions?: ApiRequestOptions): Promise<SdkWorkCommandData> {
    return this.client.request<SdkWorkCommandData>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/commands/${serializePathParameter(commandId, { name: 'commandId', style: 'simple', explode: false })}/cancel`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, sdkworkUnwrapKind: 'command' });
  }
}

export interface IotDevicesCapabilitiesListParams {
  page?: number;
  pageSize?: number;
}

export class IotDevicesCapabilitiesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List device capabilities */
  async list(deviceId: string, params?: IotDevicesCapabilitiesListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/capabilities`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface IotDevicesSessionsListParams {
  page?: number;
  pageSize?: number;
}

export class IotDevicesSessionsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List device sessions */
  async list(deviceId: string, params?: IotDevicesSessionsListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/sessions`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Disconnect device session */
  async delete(deviceId: string, sessionId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/sessions/${serializePathParameter(sessionId, { name: 'sessionId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface IotDevicesCredentialsListParams {
  page?: number;
  pageSize?: number;
}

export interface IotDevicesCredentialsCreateParams {
  idempotencyKey?: string;
}

export class IotDevicesCredentialsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List device credentials */
  async list(deviceId: string, params?: IotDevicesCredentialsListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/credentials`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Create device credential */
  async create(deviceId: string, body: AiotCredentialCreateRequest, params?: IotDevicesCredentialsCreateParams, requestOptions?: ApiRequestOptions): Promise<AiotCredentialResponse> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<AiotCredentialResponse>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/credentials`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Retrieve device credential */
  async retrieve(deviceId: string, credentialId: string, requestOptions?: ApiRequestOptions): Promise<JsonValue> {
    return this.client.request<JsonValue>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/credentials/${serializePathParameter(credentialId, { name: 'credentialId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Revoke device credential */
  async delete(deviceId: string, credentialId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/credentials/${serializePathParameter(credentialId, { name: 'credentialId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface IotDevicesListParams {
  page?: number;
  pageSize?: number;
}

export interface IotDevicesCreateParams {
  idempotencyKey?: string;
}

export interface IotDevicesUpdateParams {
  idempotencyKey?: string;
}

export class IotDevicesApi {
  private client: HttpClient;
  public readonly credentials: IotDevicesCredentialsApi;
  public readonly sessions: IotDevicesSessionsApi;
  public readonly capabilities: IotDevicesCapabilitiesApi;
  public readonly commands: IotDevicesCommandsApi;
  public readonly twin: IotDevicesTwinApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.credentials = new IotDevicesCredentialsApi(client);
    this.sessions = new IotDevicesSessionsApi(client);
    this.capabilities = new IotDevicesCapabilitiesApi(client);
    this.commands = new IotDevicesCommandsApi(client);
    this.twin = new IotDevicesTwinApi(client);
  }


/** List AIoT devices */
  async list(params?: IotDevicesListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Create AIoT device */
  async create(body: AiotDeviceCreateRequest, params?: IotDevicesCreateParams, requestOptions?: ApiRequestOptions): Promise<JsonValue> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<JsonValue>(backendApiPath(`/iot/devices`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Retrieve AIoT device */
  async retrieve(deviceId: string, requestOptions?: ApiRequestOptions): Promise<JsonValue> {
    return this.client.request<JsonValue>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Update AIoT device */
  async update(deviceId: string, body: AiotDeviceUpdateRequest, params?: IotDevicesUpdateParams, requestOptions?: ApiRequestOptions): Promise<JsonValue> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<JsonValue>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PUT' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Delete AIoT device */
  async delete(deviceId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface IotCapabilityModelsListParams {
  page?: number;
  pageSize?: number;
}

export class IotCapabilityModelsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List capability models */
  async list(params?: IotCapabilityModelsListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/capability_models`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Create capability model */
  async create(body: AiotCapabilityModelCreateRequest, requestOptions?: ApiRequestOptions): Promise<AiotCapabilityModelResponse> {
    return this.client.request<AiotCapabilityModelResponse>(backendApiPath(`/iot/capability_models`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** Retrieve capability model */
  async retrieve(capabilityModelId: string, requestOptions?: ApiRequestOptions): Promise<AiotCapabilityModelResponse> {
    return this.client.request<AiotCapabilityModelResponse>(backendApiPath(`/iot/capability_models/${serializePathParameter(capabilityModelId, { name: 'capabilityModelId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Update capability model */
  async update(capabilityModelId: string, body?: AiotCapabilityModelUpdateRequest, requestOptions?: ApiRequestOptions): Promise<AiotCapabilityModelResponse> {
    return this.client.request<AiotCapabilityModelResponse>(backendApiPath(`/iot/capability_models/${serializePathParameter(capabilityModelId, { name: 'capabilityModelId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PUT' as any, ...(body !== undefined ? { body, contentType: 'application/json' } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Delete capability model */
  async delete(capabilityModelId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/capability_models/${serializePathParameter(capabilityModelId, { name: 'capabilityModelId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface IotProtocolProfilesListParams {
  page?: number;
  pageSize?: number;
}

export class IotProtocolProfilesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List protocol profiles */
  async list(params?: IotProtocolProfilesListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/protocol_profiles`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Create protocol profile */
  async create(body: AiotProtocolProfileCreateRequest, requestOptions?: ApiRequestOptions): Promise<AiotProtocolProfileResponse> {
    return this.client.request<AiotProtocolProfileResponse>(backendApiPath(`/iot/protocol_profiles`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** Retrieve protocol profile */
  async retrieve(protocolProfileId: string, requestOptions?: ApiRequestOptions): Promise<AiotProtocolProfileResponse> {
    return this.client.request<AiotProtocolProfileResponse>(backendApiPath(`/iot/protocol_profiles/${serializePathParameter(protocolProfileId, { name: 'protocolProfileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Update protocol profile */
  async update(protocolProfileId: string, body?: AiotProtocolProfileUpdateRequest, requestOptions?: ApiRequestOptions): Promise<AiotProtocolProfileResponse> {
    return this.client.request<AiotProtocolProfileResponse>(backendApiPath(`/iot/protocol_profiles/${serializePathParameter(protocolProfileId, { name: 'protocolProfileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PUT' as any, ...(body !== undefined ? { body, contentType: 'application/json' } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Delete protocol profile */
  async delete(protocolProfileId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/protocol_profiles/${serializePathParameter(protocolProfileId, { name: 'protocolProfileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface IotHardwareProfilesListParams {
  page?: number;
  pageSize?: number;
}

export class IotHardwareProfilesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List hardware profiles */
  async list(params?: IotHardwareProfilesListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/hardware_profiles`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Create hardware profile */
  async create(body: AiotHardwareProfileCreateRequest, requestOptions?: ApiRequestOptions): Promise<AiotHardwareProfileResponse> {
    return this.client.request<AiotHardwareProfileResponse>(backendApiPath(`/iot/hardware_profiles`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** Retrieve hardware profile */
  async retrieve(hardwareProfileId: string, requestOptions?: ApiRequestOptions): Promise<AiotHardwareProfileResponse> {
    return this.client.request<AiotHardwareProfileResponse>(backendApiPath(`/iot/hardware_profiles/${serializePathParameter(hardwareProfileId, { name: 'hardwareProfileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Update hardware profile */
  async update(hardwareProfileId: string, body?: AiotHardwareProfileUpdateRequest, requestOptions?: ApiRequestOptions): Promise<AiotHardwareProfileResponse> {
    return this.client.request<AiotHardwareProfileResponse>(backendApiPath(`/iot/hardware_profiles/${serializePathParameter(hardwareProfileId, { name: 'hardwareProfileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PUT' as any, ...(body !== undefined ? { body, contentType: 'application/json' } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Delete hardware profile */
  async delete(hardwareProfileId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/hardware_profiles/${serializePathParameter(hardwareProfileId, { name: 'hardwareProfileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface IotProductsListParams {
  page?: number;
  pageSize?: number;
}

export class IotProductsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List AIoT products */
  async list(params?: IotProductsListParams, requestOptions?: ApiRequestOptions): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/products`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

/** Create AIoT product */
  async create(body: AiotProductCreateRequest, requestOptions?: ApiRequestOptions): Promise<AiotProductResponse> {
    return this.client.request<AiotProductResponse>(backendApiPath(`/iot/products`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

/** Retrieve AIoT product */
  async retrieve(productId: string, requestOptions?: ApiRequestOptions): Promise<AiotProductResponse> {
    return this.client.request<AiotProductResponse>(backendApiPath(`/iot/products/${serializePathParameter(productId, { name: 'productId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

/** Update AIoT product */
  async update(productId: string, body?: AiotProductUpdateRequest, requestOptions?: ApiRequestOptions): Promise<AiotProductResponse> {
    return this.client.request<AiotProductResponse>(backendApiPath(`/iot/products/${serializePathParameter(productId, { name: 'productId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PUT' as any, ...(body !== undefined ? { body, contentType: 'application/json' } : {}), sdkworkUnwrapKind: 'item' });
  }

/** Delete AIoT product */
  async delete(productId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/iot/products/${serializePathParameter(productId, { name: 'productId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export class IotApi {
  public readonly products: IotProductsApi;
  public readonly hardwareProfiles: IotHardwareProfilesApi;
  public readonly protocolProfiles: IotProtocolProfilesApi;
  public readonly capabilityModels: IotCapabilityModelsApi;
  public readonly devices: IotDevicesApi;
  public readonly firmwareArtifacts: IotFirmwareArtifactsApi;
  public readonly firmwareRollouts: IotFirmwareRolloutsApi;
  public readonly events: IotEventsApi;
  public readonly protocolAdapters: IotProtocolAdaptersApi;
  public readonly runtime: IotRuntimeApi;

  constructor(client: HttpClient) {
    this.products = new IotProductsApi(client);
    this.hardwareProfiles = new IotHardwareProfilesApi(client);
    this.protocolProfiles = new IotProtocolProfilesApi(client);
    this.capabilityModels = new IotCapabilityModelsApi(client);
    this.devices = new IotDevicesApi(client);
    this.firmwareArtifacts = new IotFirmwareArtifactsApi(client);
    this.firmwareRollouts = new IotFirmwareRolloutsApi(client);
    this.events = new IotEventsApi(client);
    this.protocolAdapters = new IotProtocolAdaptersApi(client);
    this.runtime = new IotRuntimeApi(client);
  }

}

export function createIotApi(client: HttpClient): IotApi {
  return new IotApi(client);
}

function appendQueryString(path: string, rawQueryString: string): string {
  const query = rawQueryString.replace(/^\?+/, '');
  if (!query) {
    return path;
  }
  return path.includes('?') ? `${path}&${query}` : `${path}?${query}`;
}

interface PathParameterSpec {
  name: string;
  style: string;
  explode: boolean;
}

function serializePathParameter(value: unknown, spec: PathParameterSpec): string {
  if (value === undefined || value === null) {
    return '';
  }

  const style = spec.style || 'simple';
  if (Array.isArray(value)) {
    return serializePathArray(spec.name, value, style, spec.explode);
  }
  if (typeof value === 'object') {
    return serializePathObject(spec.name, value as Record<string, unknown>, style, spec.explode);
  }
  return pathPrefix(spec.name, style, false) + encodePathValue(serializePathPrimitive(value));
}

function serializePathArray(name: string, values: unknown[], style: string, explode: boolean): string {
  const serialized = values
    .filter((item) => item !== undefined && item !== null)
    .map((item) => encodePathValue(serializePathPrimitive(item)));
  if (serialized.length === 0) {
    return pathPrefix(name, style, false);
  }
  if (style === 'matrix') {
    return explode
      ? serialized.map((item) => `;${name}=${item}`).join('')
      : `;${name}=${serialized.join(',')}`;
  }
  return pathPrefix(name, style, false) + serialized.join(explode ? '.' : ',');
}

function serializePathObject(name: string, value: Record<string, unknown>, style: string, explode: boolean): string {
  const entries = Object.entries(value).filter(([, entryValue]) => entryValue !== undefined && entryValue !== null);
  if (entries.length === 0) {
    return pathPrefix(name, style, true);
  }
  if (style === 'matrix') {
    return explode
      ? entries.map(([key, entryValue]) => `;${encodePathValue(key)}=${encodePathValue(serializePathPrimitive(entryValue))}`).join('')
      : `;${name}=${entries.flatMap(([key, entryValue]) => [encodePathValue(key), encodePathValue(serializePathPrimitive(entryValue))]).join(',')}`;
  }
  const serialized = explode
    ? entries.map(([key, entryValue]) => `${encodePathValue(key)}=${encodePathValue(serializePathPrimitive(entryValue))}`).join(style === 'label' ? '.' : ',')
    : entries.flatMap(([key, entryValue]) => [encodePathValue(key), encodePathValue(serializePathPrimitive(entryValue))]).join(',');
  return pathPrefix(name, style, true) + serialized;
}

function pathPrefix(name: string, style: string, _objectValue: boolean): string {
  if (style === 'label') return '.';
  if (style === 'matrix') return `;${name}`;
  return '';
}

function encodePathValue(value: string): string {
  return encodeURIComponent(value);
}

function serializePathPrimitive(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}
interface QueryParameterSpec {
  name: string;
  value: unknown;
  style: string;
  explode: boolean;
  allowReserved: boolean;
  contentType?: string;
}

function buildQueryString(parameters: QueryParameterSpec[]): string {
  const pairs: string[] = [];
  for (const parameter of parameters) {
    appendSerializedParameter(pairs, parameter);
  }
  return pairs.join('&');
}

function appendSerializedParameter(pairs: string[], parameter: QueryParameterSpec): void {
  if (parameter.value === undefined || parameter.value === null) {
    return;
  }

  if (parameter.contentType) {
    pairs.push(`${encodeQueryComponent(parameter.name)}=${encodeQueryValue(JSON.stringify(parameter.value), parameter.allowReserved)}`);
    return;
  }

  const style = parameter.style || 'form';
  if (style === 'deepObject') {
    appendDeepObjectParameter(pairs, parameter.name, parameter.value, parameter.allowReserved);
    return;
  }

  if (Array.isArray(parameter.value)) {
    appendArrayParameter(pairs, parameter.name, parameter.value, style, parameter.explode, parameter.allowReserved);
    return;
  }

  if (typeof parameter.value === 'object') {
    appendObjectParameter(pairs, parameter.name, parameter.value as Record<string, unknown>, style, parameter.explode, parameter.allowReserved);
    return;
  }

  pairs.push(`${encodeQueryComponent(parameter.name)}=${encodeQueryValue(serializePrimitive(parameter.value), parameter.allowReserved)}`);
}

function appendArrayParameter(
  pairs: string[],
  name: string,
  value: unknown[],
  style: string,
  explode: boolean,
  allowReserved: boolean,
): void {
  const values = value
    .filter((item) => item !== undefined && item !== null)
    .map((item) => serializePrimitive(item));
  if (values.length === 0) {
    return;
  }

  if (style === 'form' && explode) {
    for (const item of values) {
      pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(item, allowReserved)}`);
    }
    return;
  }

  pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(values.join(','), allowReserved)}`);
}

function appendObjectParameter(
  pairs: string[],
  name: string,
  value: Record<string, unknown>,
  style: string,
  explode: boolean,
  allowReserved: boolean,
): void {
  const entries = Object.entries(value).filter(([, entryValue]) => entryValue !== undefined && entryValue !== null);
  if (entries.length === 0) {
    return;
  }

  if (style === 'form' && explode) {
    for (const [key, entryValue] of entries) {
      pairs.push(`${encodeQueryComponent(key)}=${encodeQueryValue(serializePrimitive(entryValue), allowReserved)}`);
    }
    return;
  }

  const serialized = entries.flatMap(([key, entryValue]) => [key, serializePrimitive(entryValue)]).join(',');
  pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(serialized, allowReserved)}`);
}

function appendDeepObjectParameter(
  pairs: string[],
  name: string,
  value: unknown,
  allowReserved: boolean,
): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(serializePrimitive(value), allowReserved)}`);
    return;
  }

  for (const [key, entryValue] of Object.entries(value as Record<string, unknown>)) {
    if (entryValue === undefined || entryValue === null) {
      continue;
    }
    pairs.push(`${encodeQueryComponent(`${name}[${key}]`)}=${encodeQueryValue(serializePrimitive(entryValue), allowReserved)}`);
  }
}

function serializePrimitive(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}

function encodeQueryComponent(value: string): string {
  return encodeURIComponent(value);
}

function encodeQueryValue(value: string, allowReserved: boolean): string {
  const encoded = encodeURIComponent(value);
  if (!allowReserved) {
    return encoded;
  }
  return encoded.replace(/%3A/gi, ':')
    .replace(/%2F/gi, '/')
    .replace(/%3F/gi, '?')
    .replace(/%23/gi, '#')
    .replace(/%5B/gi, '[')
    .replace(/%5D/gi, ']')
    .replace(/%40/gi, '@')
    .replace(/%21/gi, '!')
    .replace(/%24/gi, '$')
    .replace(/%26/gi, '&')
    .replace(/%27/gi, "'")
    .replace(/%28/gi, '(')
    .replace(/%29/gi, ')')
    .replace(/%2A/gi, '*')
    .replace(/%2B/gi, '+')
    .replace(/%2C/gi, ',')
    .replace(/%3B/gi, ';')
    .replace(/%3D/gi, '=');
}
function buildRequestHeaders(
  headers: Record<string, HeaderParameterSpec | undefined>,
  cookies: Record<string, HeaderParameterSpec | undefined> = {},
): Record<string, string> | undefined {
  const requestHeaders: Record<string, string> = {};

  for (const [name, parameter] of Object.entries(headers)) {
    const serialized = serializeParameterValue(parameter);
    if (serialized !== undefined) {
      requestHeaders[name] = serialized;
    }
  }

  const cookieHeader = buildCookieHeader(cookies);
  if (cookieHeader) {
    requestHeaders.Cookie = requestHeaders.Cookie
      ? `${requestHeaders.Cookie}; ${cookieHeader}`
      : cookieHeader;
  }

  return Object.keys(requestHeaders).length > 0 ? requestHeaders : undefined;
}

interface HeaderParameterSpec {
  value: unknown;
  style: string;
  explode: boolean;
  contentType?: string;
}

function buildCookieHeader(cookies: Record<string, HeaderParameterSpec | undefined>): string | undefined {
  const pairs: string[] = [];
  for (const [name, parameter] of Object.entries(cookies)) {
    const serialized = serializeParameterValue(parameter);
    if (serialized !== undefined) {
      pairs.push(`${encodeURIComponent(name)}=${encodeURIComponent(serialized)}`);
    }
  }
  return pairs.length > 0 ? pairs.join('; ') : undefined;
}

function serializeParameterValue(parameter: HeaderParameterSpec | undefined): string | undefined {
  const value = parameter?.value;
  if (value === undefined || value === null) {
    return undefined;
  }
  if (parameter?.contentType) {
    return JSON.stringify(value);
  }
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (Array.isArray(value)) {
    return value.map((item) => serializeHeaderPrimitive(item)).join(',');
  }
  if (typeof value === 'object' && value !== null) {
    return serializeHeaderObject(value as Record<string, unknown>, parameter?.explode === true);
  }
  return serializeHeaderPrimitive(value);
}

function serializeHeaderObject(value: Record<string, unknown>, explode: boolean): string {
  const entries = Object.entries(value).filter(([, entryValue]) => entryValue !== undefined && entryValue !== null);
  if (explode) {
    return entries.map(([key, entryValue]) => `${key}=${serializeHeaderPrimitive(entryValue)}`).join(',');
  }
  return entries.flatMap(([key, entryValue]) => [key, serializeHeaderPrimitive(entryValue)]).join(',');
}

function serializeHeaderPrimitive(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString();
  }
  return String(value);
}
