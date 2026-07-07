import type {
  AiotCommand,
  AiotCommandCreateRequest,
  SdkworkAiotAppClient,
} from '@sdkwork/aiot-app-sdk';

import type { JsonValue } from '@sdkwork/aiot-app-sdk';

import { createMessageId, nowIso, readRecord, readString, sleep } from '../utils/session';

const TERMINAL_COMMAND_STATUSES = new Set(['completed', 'failed', 'cancelled', 'timeout']);

function mapCommandResource(
  item: unknown,
  deviceId: string,
  commandId: string,
): AiotCommand | null {
  const record = readRecord(item);
  const status = readString(record.status);
  if (!status || !TERMINAL_COMMAND_STATUSES.has(status)) {
    return null;
  }

  return {
    ackAt: readString(record.ackAt) || undefined,
    capabilityName: readString(record.capabilityName),
    commandId: readString(record.commandId, commandId),
    commandName: readString(record.commandName),
    createdAt: readString(record.createdAt, nowIso()),
    deviceId: readString(record.deviceId, deviceId),
    requestPayload: (record.requestPayload ?? {}) as JsonValue,
    result: record.result as AiotCommand['result'],
    resultAt: readString(record.resultAt) || undefined,
    sessionId: readString(record.sessionId) || undefined,
    status,
    traceId: readString(record.traceId) || undefined,
  };
}

export interface CreateAiotCommandServiceOptions {
  aiotClient: SdkworkAiotAppClient;
}

export interface ExecuteDeviceCommandInput {
  capabilityName: string;
  commandName: string;
  deviceId: string;
  idempotencyKey?: string;
  payload?: Record<string, unknown>;
  sessionId?: string;
}

export interface AiotCommandService {
  executeCommand(input: ExecuteDeviceCommandInput): Promise<AiotCommand>;
  speak(deviceId: string, text: string, sessionId?: string, lang?: string): Promise<AiotCommand>;
}

function mapCommandAcceptance(
  data: unknown,
  input: ExecuteDeviceCommandInput,
): AiotCommand {
  const record = readRecord(data);
  const commandId = readString(record.resourceId, readString(record.commandId));
  return {
    capabilityName: input.capabilityName,
    commandId,
    commandName: input.commandName,
    createdAt: nowIso(),
    deviceId: input.deviceId,
    requestPayload: input.payload ?? {},
    sessionId: input.sessionId,
    status: readString(record.status, 'accepted'),
  };
}

export function createAiotCommandService(
  options: CreateAiotCommandServiceOptions,
): AiotCommandService {
  const { aiotClient } = options;

  return {
    async executeCommand(input) {
      const body: AiotCommandCreateRequest = {
        capabilityName: input.capabilityName,
        commandName: input.commandName,
        payload: input.payload ?? {},
        ...(input.sessionId ? { sessionId: input.sessionId } : {}),
      };

      const acceptance = await aiotClient.iot.devices.commands.create(
        input.deviceId,
        body,
        input.idempotencyKey ? { idempotencyKey: input.idempotencyKey } : undefined,
      );

      return mapCommandAcceptance(acceptance, input);
    },

    async speak(deviceId, text, sessionId, lang = 'zh-CN') {
      return this.executeCommand({
        capabilityName: 'audio.playback',
        commandName: 'speak',
        deviceId,
        payload: { lang, text },
        sessionId,
      });
    },
  };
}

export async function pollCommandResult(
  aiotClient: SdkworkAiotAppClient,
  deviceId: string,
  commandId: string,
  options: { intervalMs?: number; maxAttempts?: number } = {},
): Promise<AiotCommand | null> {
  const intervalMs = options.intervalMs ?? 400;
  const maxAttempts = options.maxAttempts ?? 12;

  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const item = await aiotClient.iot.devices.commands.retrieve(deviceId, commandId);
    const command = mapCommandResource(item, deviceId, commandId);
    if (command) {
      return command;
    }

    await sleep(intervalMs);
  }

  return null;
}

export { createMessageId };
