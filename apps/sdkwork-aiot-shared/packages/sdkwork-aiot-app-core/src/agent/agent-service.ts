import type { SdkworkAiotAppClient } from '@sdkwork/aiot-app-sdk';

import {
  createAiotCommandService,
  pollCommandResult,
  type AiotCommandService,
} from '../command/command-service';
import type { AiotAgentsDialoguePort } from '../ports/dialogue-ports';
import type {
  AiotAgentToolCall,
  AiotConversationMessage,
  AiotConversationSession,
} from '../types/conversation';
import { createMessageId, createSessionId, nowIso, readRecord, readString } from '../utils/session';

export interface CreateAiotAgentServiceOptions {
  agentsDialoguePort?: AiotAgentsDialoguePort;
  aiotClient: SdkworkAiotAppClient;
  commandService?: AiotCommandService;
  /** When true (default), device `assistant.chat` is used if sdkwork-agents fails. */
  fallbackToDeviceOnAgentsFailure?: boolean;
}

export interface SendAgentMessageInput {
  deviceId: string;
  sessionId?: string;
  text: string;
}

export interface AiotAgentService {
  createSession(deviceId: string, title?: string): AiotConversationSession;
  getMessages(sessionId: string): AiotConversationMessage[];
  getSessions(): AiotConversationSession[];
  getToolCalls(sessionId: string): AiotAgentToolCall[];
  sendMessage(input: SendAgentMessageInput): Promise<AiotConversationMessage>;
}

function extractAssistantText(resultPayload: unknown): string | null {
  const record = readRecord(resultPayload);
  const text = readString(record.text) || readString(record.reply) || readString(record.message);
  return text || null;
}

export function createAiotAgentService(
  options: CreateAiotAgentServiceOptions,
): AiotAgentService {
  const commandService = options.commandService ?? createAiotCommandService(options);
  const agentsDialoguePort = options.agentsDialoguePort;
  const fallbackToDeviceOnAgentsFailure = options.fallbackToDeviceOnAgentsFailure !== false;
  const sessions = new Map<string, AiotConversationSession>();
  const messages = new Map<string, AiotConversationMessage[]>();
  const toolCalls = new Map<string, AiotAgentToolCall[]>();

  function ensureSession(deviceId: string, sessionId: string, title?: string): AiotConversationSession {
    const existing = sessions.get(sessionId);
    if (existing) {
      return existing;
    }

    const session: AiotConversationSession = {
      createdAt: nowIso(),
      deviceId,
      id: sessionId,
      title: title?.trim() || 'AIoT 智能体会话',
      updatedAt: nowIso(),
    };

    sessions.set(session.id, session);
    messages.set(session.id, []);
    toolCalls.set(session.id, []);
    return session;
  }

  async function sendViaAgentsPort(
    session: AiotConversationSession,
    text: string,
  ): Promise<string> {
    if (!agentsDialoguePort?.configured) {
      throw new Error('sdkwork-agents dialogue port is not configured.');
    }

    const agentId = session.agentsAgentId ?? agentsDialoguePort.resolveAgentId(session.deviceId);
    let remoteSessionId = session.agentsSessionId;
    if (!remoteSessionId) {
      remoteSessionId = await agentsDialoguePort.createRemoteSession(agentId, session.title);
      session.agentsAgentId = agentId;
      session.agentsSessionId = remoteSessionId;
      session.updatedAt = nowIso();
      sessions.set(session.id, session);
    }

    return agentsDialoguePort.sendChat({
      agentId,
      remoteSessionId,
      text,
    });
  }

  async function sendViaDeviceCommand(
    sessionId: string,
    deviceId: string,
    text: string,
    sessionMessages: AiotConversationMessage[],
  ): Promise<string> {
    const command = await commandService.executeCommand({
      capabilityName: 'assistant',
      commandName: 'chat',
      deviceId,
      payload: {
        history: sessionMessages
          .filter((message) => message.status === 'completed')
          .map((message) => ({ content: message.content, role: message.role })),
        lang: 'zh-CN',
        text,
      },
      sessionId,
    });

    const completed = await pollCommandResult(options.aiotClient, deviceId, command.commandId);
    const replyText = extractAssistantText(completed?.result?.resultPayload);
    if (!replyText) {
      throw new Error(
        '设备未返回 assistant.chat 回复，请确认设备在线且已启用智能体能力。',
      );
    }
    return replyText;
  }

  return {
    createSession(deviceId, title) {
      return ensureSession(deviceId, createSessionId('conv'), title);
    },

    getMessages(sessionId) {
      return [...(messages.get(sessionId) ?? [])];
    },

    getSessions() {
      return [...sessions.values()].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
    },

    getToolCalls(sessionId) {
      return [...(toolCalls.get(sessionId) ?? [])];
    },

    async sendMessage(input) {
      const sessionId = input.sessionId ?? createSessionId('conv');
      ensureSession(input.deviceId, sessionId);

      const userMessage: AiotConversationMessage = {
        content: input.text.trim(),
        createdAt: nowIso(),
        id: createMessageId('user'),
        role: 'user',
        sessionId,
        status: 'completed',
      };

      const sessionMessages = messages.get(sessionId) ?? [];
      sessionMessages.push(userMessage);
      messages.set(sessionId, sessionMessages);

      const pendingAssistant: AiotConversationMessage = {
        content: '',
        createdAt: nowIso(),
        id: createMessageId('assistant'),
        role: 'assistant',
        sessionId,
        status: 'pending',
      };
      sessionMessages.push(pendingAssistant);

      try {
        const session = sessions.get(sessionId);
        if (!session) {
          throw new Error('Conversation session not found.');
        }

        let replyText: string | null = null;
        if (agentsDialoguePort?.configured) {
          try {
            replyText = await sendViaAgentsPort(session, input.text.trim());
          } catch (agentsError) {
            if (!fallbackToDeviceOnAgentsFailure) {
              throw agentsError;
            }
            replyText = await sendViaDeviceCommand(
              sessionId,
              input.deviceId,
              input.text.trim(),
              sessionMessages,
            );
          }
        } else {
          replyText = await sendViaDeviceCommand(
            sessionId,
            input.deviceId,
            input.text.trim(),
            sessionMessages,
          );
        }

        pendingAssistant.content = replyText;
        pendingAssistant.status = 'completed';
        pendingAssistant.createdAt = nowIso();

        session.updatedAt = nowIso();
        sessions.set(sessionId, session);

        return pendingAssistant;
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'AIoT command execution failed';
        pendingAssistant.content = message;
        pendingAssistant.status = 'failed';
        pendingAssistant.createdAt = nowIso();
        throw error;
      }
    },
  };
}
