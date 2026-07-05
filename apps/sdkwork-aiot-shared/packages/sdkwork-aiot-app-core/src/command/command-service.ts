import type {
  AiotCommand,
  AiotCommandCreateRequest,
  SdkworkAiotAppClient,
} from '@sdkwork/aiot-app-sdk';

import { createMessageId, nowIso, readRecord, readString, sleep } from '../utils/session';

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
        input.idempotencyKey,
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
    let page = 1;
    while (true) {
      const eventsPage = await aiotClient.iot.devices.events.list(deviceId, {
        page,
        page_size: 200,
        q: commandId,
      });
      const items = Array.isArray(eventsPage.items) ? eventsPage.items : [];
      const match = items.find((event) => {
        const payload = readRecord(event.payload);
        const correlationId = readString(payload.correlationId) || readString(payload.commandId);
        return correlationId === commandId;
      });

      if (match) {
        const payload = readRecord(match.payload);
        return {
          ackAt: readString(payload.ackAt) || undefined,
          capabilityName: readString(payload.capabilityName),
          commandId,
          commandName: readString(payload.commandName),
          createdAt: readString(match.occurredAt, nowIso()),
          deviceId,
          requestPayload: payload.requestPayload ?? {},
          result: payload.result as AiotCommand['result'],
          resultAt: readString(payload.resultAt) || undefined,
          sessionId: readString(payload.sessionId) || undefined,
          status: readString(payload.status, 'completed'),
          traceId: readString(payload.traceId) || undefined,
        };
      }

      const pageInfo = readRecord(eventsPage.pageInfo);
      if (!pageInfo.hasMore || items.length === 0) {
        break;
      }
      page += 1;
    }

    await sleep(intervalMs);
  }

  return null;
}

export function createLocalAssistantReply(userText: string): string {
  const trimmed = userText.trim();
  if (!trimmed) {
    return '请告诉我您需要什么帮助。';
  }

  return `已收到您的指令：「${trimmed}」。AIoT 智能体正在处理设备联动与场景编排。`;
}

export { createMessageId };
