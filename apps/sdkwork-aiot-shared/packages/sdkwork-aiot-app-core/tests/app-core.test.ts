import { describe, expect, it, vi } from 'vitest';

import {
  createAiotAgentService,
  createAiotCommandService,
  createAiotVoiceDialogueService,
  pollCommandResult,
} from '../src';

describe('aiot-app-core command service', () => {
  it('sends speak commands through sdkwork-aiot-app-sdk', async () => {
    const create = vi.fn().mockResolvedValue({
      accepted: true,
      resourceId: 'cmd-1',
      status: 'accepted',
    });

    const service = createAiotCommandService({
      aiotClient: {
        iot: {
          devices: {
            commands: {
              create,
            },
          },
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

  it('polls command completion through devices.commands.retrieve', async () => {
    const retrieve = vi
      .fn()
      .mockResolvedValueOnce({ status: 'accepted' })
      .mockResolvedValueOnce({
        commandId: 'cmd-1',
        deviceId: 'dev-1',
        status: 'completed',
        capabilityName: 'audio.playback',
        commandName: 'speak',
        createdAt: '2026-07-06T00:00:00.000Z',
        requestPayload: {},
        result: { resultPayload: { text: 'done' } },
      });

    const result = await pollCommandResult(
      {
        iot: {
          devices: {
            commands: {
              retrieve,
            },
          },
        },
      } as never,
      'dev-1',
      'cmd-1',
      { intervalMs: 0, maxAttempts: 3 },
    );

    expect(retrieve).toHaveBeenCalledWith('dev-1', 'cmd-1');
    expect(result?.status).toBe('completed');
  });
});

describe('aiot-app-core agent service', () => {
  it('uses sdkwork-agents dialogue port when configured', async () => {
    const service = createAiotAgentService({
      agentsDialoguePort: {
        configured: true,
        resolveAgentId: () => 'agent.aiot.assistant',
        createRemoteSession: vi.fn().mockResolvedValue('remote-session-1'),
        sendChat: vi.fn().mockResolvedValue('agents reply'),
      },
      aiotClient: {
        iot: { devices: { commands: { create: vi.fn() }, events: { list: vi.fn() } } },
      } as never,
    });

    const session = service.createSession('dev-1');
    const reply = await service.sendMessage({
      deviceId: 'dev-1',
      sessionId: session.id,
      text: '打开客厅灯',
    });

    expect(reply.content).toBe('agents reply');
    expect(reply.status).toBe('completed');
  });

  it('falls back to device assistant.chat when sdkwork-agents fails', async () => {
    const create = vi.fn().mockResolvedValue({
      accepted: true,
      resourceId: 'cmd-fallback',
      status: 'accepted',
    });
    const list = vi.fn().mockResolvedValue({
      items: [{
        payload: {
          commandId: 'cmd-fallback',
          result: { resultPayload: { text: 'device fallback reply' } },
          status: 'completed',
        },
      }],
      pageInfo: { page: 1, pageSize: 20, total: 1, hasMore: false },
    });

    const service = createAiotAgentService({
      agentsDialoguePort: {
        configured: true,
        resolveAgentId: () => 'agent.aiot.assistant',
        createRemoteSession: vi.fn().mockResolvedValue('remote-session-1'),
        sendChat: vi.fn().mockRejectedValue(new Error('agents offline')),
      },
      aiotClient: {
        iot: {
          devices: {
            commands: { create },
            events: { list },
          },
        },
      } as never,
    });

    const session = service.createSession('dev-1');
    const reply = await service.sendMessage({
      deviceId: 'dev-1',
      sessionId: session.id,
      text: '打开客厅灯',
    });

    expect(reply.content).toBe('device fallback reply');
    expect(create).toHaveBeenCalled();
  });

  it('marks assistant messages failed when device returns no reply payload', async () => {
    const service = createAiotAgentService({
      aiotClient: {
        iot: {
          devices: {
            commands: {
              create: vi.fn().mockResolvedValue({
                accepted: true,
                resourceId: 'cmd-empty',
                status: 'accepted',
              }),
            },
            events: {
              list: vi.fn().mockResolvedValue({ items: [], pageInfo: { page: 1, pageSize: 20, total: 0, hasMore: false } }),
            },
          },
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
          devices: {
            commands: {
              create: vi.fn().mockRejectedValue(new Error('offline')),
            },
          },
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

describe('aiot-app-core voice dialogue service', () => {
  it('prefers online device speak over cloud TTS', async () => {
    const speakOnDevice = vi.fn().mockResolvedValue(undefined);
    const speakViaCloud = vi.fn().mockResolvedValue(undefined);

    const voiceService = {
      isCloudVoiceConfigured: () => true,
      isListening: () => false,
      listVoiceDevices: vi.fn().mockResolvedValue([
        { deviceId: 'dev-1', displayName: 'Speaker', online: true, status: 'online' },
      ]),
      speakOnDevice,
      speakLocally: vi.fn(),
      speakViaCloud,
      startListening: vi.fn(),
      stopListening: vi.fn(),
    };

    const agentService = {
      createSession: vi.fn().mockReturnValue({ id: 'local-session' }),
      getMessages: vi.fn().mockReturnValue([]),
      getSessions: vi.fn().mockReturnValue([]),
      getToolCalls: vi.fn().mockReturnValue([]),
      sendMessage: vi.fn().mockResolvedValue({
        content: 'hello from agent',
        createdAt: '2026-01-01T00:00:00Z',
        id: 'assistant-1',
        role: 'assistant',
        sessionId: 'local-session',
        status: 'completed',
      }),
    };

    const dialogue = createAiotVoiceDialogueService({
      agentService: agentService as never,
      agentsDialoguePort: { configured: true, resolveAgentId: () => 'agent-1', createRemoteSession: vi.fn(), sendChat: vi.fn() },
      voiceDialoguePort: { configured: true, synthesize: vi.fn(), transcribe: vi.fn() },
      voiceService: voiceService as never,
    });

    await dialogue.runDialogueTurn('打开客厅灯');
    expect(speakOnDevice).toHaveBeenCalledWith('dev-1', 'hello from agent', expect.any(String));
    expect(speakViaCloud).not.toHaveBeenCalled();
  });

  it('invokes onDialogueComplete after auto-run listen turn', async () => {
    const onDialogueComplete = vi.fn();
    const agentService = {
      createSession: vi.fn().mockReturnValue({ id: 'local-session' }),
      getMessages: vi.fn().mockReturnValue([]),
      getSessions: vi.fn().mockReturnValue([]),
      getToolCalls: vi.fn().mockReturnValue([]),
      sendMessage: vi.fn().mockResolvedValue({
        content: 'auto reply',
        createdAt: '2026-01-01T00:00:00Z',
        id: 'assistant-1',
        role: 'assistant',
        sessionId: 'local-session',
        status: 'completed',
      }),
    };

    const voiceService = {
      isCloudVoiceConfigured: () => false,
      isListening: () => false,
      listVoiceDevices: vi.fn().mockResolvedValue([]),
      speakOnDevice: vi.fn(),
      speakLocally: vi.fn().mockResolvedValue(undefined),
      speakViaCloud: vi.fn(),
      startListening: vi.fn(async (onResult) => {
        await onResult('打开灯', true);
      }),
      stopListening: vi.fn(),
    };

    const dialogue = createAiotVoiceDialogueService({
      agentService: agentService as never,
      voiceService: voiceService as never,
    });

    await dialogue.startListening(vi.fn(), {
      autoRunDialogue: true,
      onDialogueComplete,
    });

    expect(onDialogueComplete).toHaveBeenCalledWith('auto reply');
  });
});
