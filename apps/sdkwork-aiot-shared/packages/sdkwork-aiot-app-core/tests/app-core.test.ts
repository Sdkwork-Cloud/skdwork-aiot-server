import { describe, expect, it, vi } from 'vitest';

import {
  createAiotAgentService,
  createAiotCommandService,
  createLocalAssistantReply,
} from '../src';

describe('aiot-app-core command service', () => {
  it('sends speak commands through sdkwork-aiot-app-sdk', async () => {
    const create = vi.fn().mockResolvedValue({
      code: '202',
      data: {
        commandId: 'cmd-1',
        deviceId: 'dev-1',
        capabilityName: 'audio.playback',
        commandName: 'speak',
        requestPayload: { text: 'hello', lang: 'zh-CN' },
        status: 'accepted',
        createdAt: '2026-06-18T00:00:00.000Z',
      },
    });

    const service = createAiotCommandService({
      aiotClient: {
        iot: {
          devicesCommandsCreate: create,
        },
      } as never,
    });

    const command = await service.speak('dev-1', 'hello');

    expect(create).toHaveBeenCalledWith(
      'dev-1',
      {
        capabilityName: 'audio.playback',
        commandName: 'speak',
        payload: { lang: 'zh-CN', text: 'hello' },
      },
      undefined,
    );
    expect(command.commandId).toBe('cmd-1');
  });
});

describe('aiot-app-core agent service', () => {
  it('marks assistant messages failed when device returns no reply payload', async () => {
    const service = createAiotAgentService({
      aiotClient: {
        iot: {
          devicesCommandsCreate: vi.fn().mockResolvedValue({
            code: '202',
            data: {
              commandId: 'cmd-empty',
              deviceId: 'dev-1',
              capabilityName: 'assistant',
              commandName: 'chat',
              status: 'accepted',
              createdAt: '2026-06-18T00:00:00.000Z',
            },
          }),
          devicesEventsList: vi.fn().mockResolvedValue({ data: [] }),
        },
      } as never,
    });

    const session = service.createSession('dev-1');
    await expect(
      service.sendMessage({
        deviceId: 'dev-1',
        sessionId: session.id,
        text: '打开客厅灯',
      }),
    ).rejects.toThrow('assistant.chat');

    const messages = service.getMessages(session.id);
    expect(messages[1]?.status).toBe('failed');
  });

  it('marks assistant messages failed when command execution fails', async () => {
    const service = createAiotAgentService({
      aiotClient: {
        iot: {
          devicesCommandsCreate: vi.fn().mockRejectedValue(new Error('offline')),
        },
      } as never,
    });

    const session = service.createSession('dev-1');
    await expect(
      service.sendMessage({
        deviceId: 'dev-1',
        sessionId: session.id,
        text: '打开客厅灯',
      }),
    ).rejects.toThrow('offline');

    const messages = service.getMessages(session.id);
    expect(messages).toHaveLength(2);
    expect(messages[1]?.role).toBe('assistant');
    expect(messages[1]?.status).toBe('failed');
    expect(messages[1]?.content).toContain('offline');
  });
});

describe('createLocalAssistantReply', () => {
  it('returns a helpful default for empty input', () => {
    expect(createLocalAssistantReply('')).toContain('帮助');
  });
});
