import { backendApiPath } from './paths';
import type { HttpClient } from '../http/client';

import type { AiotCapabilityModelCreateRequest, AiotCapabilityModelResponse, AiotCapabilityModelUpdateRequest, AiotCredentialCreateRequest, AiotCredentialResponse, AiotDeviceCreateRequest, AiotDeviceUpdateRequest, AiotFirmwareArtifactCreateRequest, AiotFirmwareArtifactResponse, AiotFirmwareArtifactUpdateRequest, AiotFirmwareRolloutCreateRequest, AiotFirmwareRolloutResponse, AiotFirmwareRolloutUpdateRequest, AiotHardwareProfileCreateRequest, AiotHardwareProfileResponse, AiotHardwareProfileUpdateRequest, AiotProductCreateRequest, AiotProductResponse, AiotProductUpdateRequest, AiotProtocolProfileCreateRequest, AiotProtocolProfileResponse, AiotProtocolProfileUpdateRequest, AiotRuntimeCapacityPolicyResponse, AiotTwinUpdateRequest, SdkWorkCommandData, SdkWorkPageData, StandardResourceResponse } from '../types';


export class IotRuntimeCapacityApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Retrieve AIoT runtime capacity and backpressure policy */
  async retrieve(): Promise<AiotRuntimeCapacityPolicyResponse> {
    return this.client.get<AiotRuntimeCapacityPolicyResponse>(backendApiPath(`/iot/runtime/capacity`));
  }
}

export class IotRuntimeApi {
  private client: HttpClient;
  public readonly capacity: IotRuntimeCapacityApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.capacity = new IotRuntimeCapacityApi(client);
  }

}

export interface IotProtocolAdaptersListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotProtocolAdaptersApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List protocol adapters */
  async list(params?: IotProtocolAdaptersListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/protocol_adapters`), query));
  }
}

export interface IotEventsListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotEventsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List platform IoT events */
  async list(params?: IotEventsListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/events`), query));
  }
}

export interface IotFirmwareRolloutsListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
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
  async list(params?: IotFirmwareRolloutsListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/firmware_rollouts`), query));
  }

/** Create firmware rollout */
  async create(body: AiotFirmwareRolloutCreateRequest, params?: IotFirmwareRolloutsCreateParams): Promise<AiotFirmwareRolloutResponse> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.post<AiotFirmwareRolloutResponse>(backendApiPath(`/iot/firmware_rollouts`), body, undefined, requestHeaders, 'application/json');
  }

/** Retrieve firmware rollout */
  async retrieve(rolloutId: string): Promise<AiotFirmwareRolloutResponse> {
    return this.client.get<AiotFirmwareRolloutResponse>(backendApiPath(`/iot/firmware_rollouts/${serializePathParameter(rolloutId, { name: 'rolloutId', style: 'simple', explode: false })}`));
  }

/** Update firmware rollout */
  async update(rolloutId: string, body?: AiotFirmwareRolloutUpdateRequest): Promise<AiotFirmwareRolloutResponse> {
    return this.client.put<AiotFirmwareRolloutResponse>(backendApiPath(`/iot/firmware_rollouts/${serializePathParameter(rolloutId, { name: 'rolloutId', style: 'simple', explode: false })}`), body, undefined, undefined, 'application/json');
  }

/** Delete firmware rollout */
  async delete(rolloutId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/firmware_rollouts/${serializePathParameter(rolloutId, { name: 'rolloutId', style: 'simple', explode: false })}`));
  }
}

export interface IotFirmwareArtifactsListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
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
  async list(params?: IotFirmwareArtifactsListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/firmware_artifacts`), query));
  }

/** Create firmware artifact metadata */
  async create(body: AiotFirmwareArtifactCreateRequest, params?: IotFirmwareArtifactsCreateParams): Promise<AiotFirmwareArtifactResponse> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.post<AiotFirmwareArtifactResponse>(backendApiPath(`/iot/firmware_artifacts`), body, undefined, requestHeaders, 'application/json');
  }

/** Retrieve firmware artifact */
  async retrieve(artifactId: string): Promise<AiotFirmwareArtifactResponse> {
    return this.client.get<AiotFirmwareArtifactResponse>(backendApiPath(`/iot/firmware_artifacts/${serializePathParameter(artifactId, { name: 'artifactId', style: 'simple', explode: false })}`));
  }

/** Update firmware artifact metadata */
  async update(artifactId: string, body?: AiotFirmwareArtifactUpdateRequest): Promise<AiotFirmwareArtifactResponse> {
    return this.client.put<AiotFirmwareArtifactResponse>(backendApiPath(`/iot/firmware_artifacts/${serializePathParameter(artifactId, { name: 'artifactId', style: 'simple', explode: false })}`), body, undefined, undefined, 'application/json');
  }

/** Delete firmware artifact metadata */
  async delete(artifactId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/firmware_artifacts/${serializePathParameter(artifactId, { name: 'artifactId', style: 'simple', explode: false })}`));
  }
}

export class IotDevicesTwinApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Update backend device twin */
  async update(deviceId: string, body: AiotTwinUpdateRequest): Promise<StandardResourceResponse> {
    return this.client.patch<StandardResourceResponse>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/twin`), body, undefined, undefined, 'application/json');
  }

/** Retrieve backend device twin */
  async retrieve(deviceId: string): Promise<StandardResourceResponse> {
    return this.client.get<StandardResourceResponse>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/twin`));
  }
}

export interface IotDevicesCommandsListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotDevicesCommandsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List device commands */
  async list(deviceId: string, params?: IotDevicesCommandsListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/commands`), query));
  }

/** Cancel device command */
  async cancel(deviceId: string, commandId: string): Promise<SdkWorkCommandData> {
    return this.client.post<SdkWorkCommandData>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/commands/${serializePathParameter(commandId, { name: 'commandId', style: 'simple', explode: false })}/cancel`));
  }
}

export interface IotDevicesCapabilitiesListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotDevicesCapabilitiesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List device capabilities */
  async list(deviceId: string, params?: IotDevicesCapabilitiesListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/capabilities`), query));
  }
}

export interface IotDevicesSessionsListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotDevicesSessionsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List device sessions */
  async list(deviceId: string, params?: IotDevicesSessionsListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/sessions`), query));
  }

/** Disconnect device session */
  async disconnect(deviceId: string, sessionId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/sessions/${serializePathParameter(sessionId, { name: 'sessionId', style: 'simple', explode: false })}`));
  }
}

export interface IotDevicesCredentialsListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
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
  async list(deviceId: string, params?: IotDevicesCredentialsListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/credentials`), query));
  }

/** Create device credential */
  async create(deviceId: string, body: AiotCredentialCreateRequest, params?: IotDevicesCredentialsCreateParams): Promise<AiotCredentialResponse> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.post<AiotCredentialResponse>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/credentials`), body, undefined, requestHeaders, 'application/json');
  }

/** Retrieve device credential */
  async retrieve(deviceId: string, credentialId: string): Promise<StandardResourceResponse> {
    return this.client.get<StandardResourceResponse>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/credentials/${serializePathParameter(credentialId, { name: 'credentialId', style: 'simple', explode: false })}`));
  }

/** Revoke device credential */
  async delete(deviceId: string, credentialId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}/credentials/${serializePathParameter(credentialId, { name: 'credentialId', style: 'simple', explode: false })}`));
  }
}

export interface IotDevicesListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
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
  async list(params?: IotDevicesListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/devices`), query));
  }

/** Create AIoT device */
  async create(body: AiotDeviceCreateRequest, params?: IotDevicesCreateParams): Promise<StandardResourceResponse> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.post<StandardResourceResponse>(backendApiPath(`/iot/devices`), body, undefined, requestHeaders, 'application/json');
  }

/** Retrieve AIoT device */
  async retrieve(deviceId: string): Promise<StandardResourceResponse> {
    return this.client.get<StandardResourceResponse>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}`));
  }

/** Update AIoT device */
  async update(deviceId: string, body: AiotDeviceUpdateRequest, params?: IotDevicesUpdateParams): Promise<StandardResourceResponse> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.put<StandardResourceResponse>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}`), body, undefined, requestHeaders, 'application/json');
  }

/** Delete AIoT device */
  async delete(deviceId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/devices/${serializePathParameter(deviceId, { name: 'deviceId', style: 'simple', explode: false })}`));
  }
}

export interface IotCapabilityModelsListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotCapabilityModelsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List capability models */
  async list(params?: IotCapabilityModelsListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/capability_models`), query));
  }

/** Create capability model */
  async create(body: AiotCapabilityModelCreateRequest): Promise<AiotCapabilityModelResponse> {
    return this.client.post<AiotCapabilityModelResponse>(backendApiPath(`/iot/capability_models`), body, undefined, undefined, 'application/json');
  }

/** Retrieve capability model */
  async retrieve(capabilityModelId: string): Promise<AiotCapabilityModelResponse> {
    return this.client.get<AiotCapabilityModelResponse>(backendApiPath(`/iot/capability_models/${serializePathParameter(capabilityModelId, { name: 'capabilityModelId', style: 'simple', explode: false })}`));
  }

/** Update capability model */
  async update(capabilityModelId: string, body?: AiotCapabilityModelUpdateRequest): Promise<AiotCapabilityModelResponse> {
    return this.client.put<AiotCapabilityModelResponse>(backendApiPath(`/iot/capability_models/${serializePathParameter(capabilityModelId, { name: 'capabilityModelId', style: 'simple', explode: false })}`), body, undefined, undefined, 'application/json');
  }

/** Delete capability model */
  async delete(capabilityModelId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/capability_models/${serializePathParameter(capabilityModelId, { name: 'capabilityModelId', style: 'simple', explode: false })}`));
  }
}

export interface IotProtocolProfilesListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotProtocolProfilesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List protocol profiles */
  async list(params?: IotProtocolProfilesListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/protocol_profiles`), query));
  }

/** Create protocol profile */
  async create(body: AiotProtocolProfileCreateRequest): Promise<AiotProtocolProfileResponse> {
    return this.client.post<AiotProtocolProfileResponse>(backendApiPath(`/iot/protocol_profiles`), body, undefined, undefined, 'application/json');
  }

/** Retrieve protocol profile */
  async retrieve(protocolProfileId: string): Promise<AiotProtocolProfileResponse> {
    return this.client.get<AiotProtocolProfileResponse>(backendApiPath(`/iot/protocol_profiles/${serializePathParameter(protocolProfileId, { name: 'protocolProfileId', style: 'simple', explode: false })}`));
  }

/** Update protocol profile */
  async update(protocolProfileId: string, body?: AiotProtocolProfileUpdateRequest): Promise<AiotProtocolProfileResponse> {
    return this.client.put<AiotProtocolProfileResponse>(backendApiPath(`/iot/protocol_profiles/${serializePathParameter(protocolProfileId, { name: 'protocolProfileId', style: 'simple', explode: false })}`), body, undefined, undefined, 'application/json');
  }

/** Delete protocol profile */
  async delete(protocolProfileId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/protocol_profiles/${serializePathParameter(protocolProfileId, { name: 'protocolProfileId', style: 'simple', explode: false })}`));
  }
}

export interface IotHardwareProfilesListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotHardwareProfilesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List hardware profiles */
  async list(params?: IotHardwareProfilesListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/hardware_profiles`), query));
  }

/** Create hardware profile */
  async create(body: AiotHardwareProfileCreateRequest): Promise<AiotHardwareProfileResponse> {
    return this.client.post<AiotHardwareProfileResponse>(backendApiPath(`/iot/hardware_profiles`), body, undefined, undefined, 'application/json');
  }

/** Retrieve hardware profile */
  async retrieve(hardwareProfileId: string): Promise<AiotHardwareProfileResponse> {
    return this.client.get<AiotHardwareProfileResponse>(backendApiPath(`/iot/hardware_profiles/${serializePathParameter(hardwareProfileId, { name: 'hardwareProfileId', style: 'simple', explode: false })}`));
  }

/** Update hardware profile */
  async update(hardwareProfileId: string, body?: AiotHardwareProfileUpdateRequest): Promise<AiotHardwareProfileResponse> {
    return this.client.put<AiotHardwareProfileResponse>(backendApiPath(`/iot/hardware_profiles/${serializePathParameter(hardwareProfileId, { name: 'hardwareProfileId', style: 'simple', explode: false })}`), body, undefined, undefined, 'application/json');
  }

/** Delete hardware profile */
  async delete(hardwareProfileId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/hardware_profiles/${serializePathParameter(hardwareProfileId, { name: 'hardwareProfileId', style: 'simple', explode: false })}`));
  }
}

export interface IotProductsListParams {
  page?: number;
  pageSize?: number;
  cursor?: string;
  sort?: string;
  q?: string;
}

export class IotProductsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List AIoT products */
  async list(params?: IotProductsListParams): Promise<SdkWorkPageData> {
    const query = buildQueryString([
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'sort', value: params?.sort, style: 'form', explode: true, allowReserved: false },
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.get<SdkWorkPageData>(appendQueryString(backendApiPath(`/iot/products`), query));
  }

/** Create AIoT product */
  async create(body: AiotProductCreateRequest): Promise<AiotProductResponse> {
    return this.client.post<AiotProductResponse>(backendApiPath(`/iot/products`), body, undefined, undefined, 'application/json');
  }

/** Retrieve AIoT product */
  async retrieve(productId: string): Promise<AiotProductResponse> {
    return this.client.get<AiotProductResponse>(backendApiPath(`/iot/products/${serializePathParameter(productId, { name: 'productId', style: 'simple', explode: false })}`));
  }

/** Update AIoT product */
  async update(productId: string, body?: AiotProductUpdateRequest): Promise<AiotProductResponse> {
    return this.client.put<AiotProductResponse>(backendApiPath(`/iot/products/${serializePathParameter(productId, { name: 'productId', style: 'simple', explode: false })}`), body, undefined, undefined, 'application/json');
  }

/** Delete AIoT product */
  async delete(productId: string): Promise<void> {
    return this.client.delete<void>(backendApiPath(`/iot/products/${serializePathParameter(productId, { name: 'productId', style: 'simple', explode: false })}`));
  }
}

export class IotApi {
  private client: HttpClient;
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
    this.client = client;
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
