import { describe, expect, it, vi } from 'vitest';

import {
  createAiotAgentService,
  createAiotCommandService,
  createAiotVoiceDialogueService,
  createLocalAssistantReply,
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
});

describe('aiot-app-core local assistant reply', () => {
  it('returns a helpful default for empty input', () => {
    expect(createLocalAssistantReply('   ')).toContain('请告诉我');
  });
});
