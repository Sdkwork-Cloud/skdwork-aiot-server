import type { SdkworkAiotAppClient } from '@sdkwork/aiot-app-sdk';

import {
  createAiotCommandService,
  pollCommandResult,
  type AiotCommandService,
} from '../command/command-service';
import type {
  AiotAgentToolCall,
  AiotConversationMessage,
  AiotConversationSession,
} from '../types/conversation';
import { createMessageId, createSessionId, nowIso, readRecord, readString } from '../utils/session';

export interface CreateAiotAgentServiceOptions {
  aiotClient: SdkworkAiotAppClient;
  commandService?: AiotCommandService;
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
  const sessions = new Map<string, AiotConversationSession>();
  const messages = new Map<string, AiotConversationMessage[]>();
  const toolCalls = new Map<string, AiotAgentToolCall[]>();

  return {
    createSession(deviceId, title) {
      const session: AiotConversationSession = {
        createdAt: nowIso(),
        deviceId,
        id: createSessionId('conv'),
        title: title?.trim() || 'AIoT 智能体会话',
        updatedAt: nowIso(),
      };

      sessions.set(session.id, session);
      messages.set(session.id, []);
      toolCalls.set(session.id, []);
      return session;
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
      if (!sessions.has(sessionId)) {
        this.createSession(input.deviceId);
      }

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
        const command = await commandService.executeCommand({
          capabilityName: 'assistant',
          commandName: 'chat',
          deviceId: input.deviceId,
          payload: {
            history: sessionMessages
              .filter((message) => message.status === 'completed')
              .map((message) => ({ content: message.content, role: message.role })),
            lang: 'zh-CN',
            text: input.text.trim(),
          },
          sessionId,
        });

        const completed = await pollCommandResult(options.aiotClient, input.deviceId, command.commandId);
        const replyText = extractAssistantText(completed?.result?.resultPayload);
        if (!replyText) {
          const missingReplyError = new Error(
            '设备未返回 assistant.chat 回复，请确认设备在线且已启用智能体能力。',
          );
          pendingAssistant.content = missingReplyError.message;
          pendingAssistant.status = 'failed';
          pendingAssistant.createdAt = nowIso();
          throw missingReplyError;
        }

        pendingAssistant.content = replyText;
        pendingAssistant.status = 'completed';
        pendingAssistant.createdAt = nowIso();

        const session = sessions.get(sessionId);
        if (session) {
          session.updatedAt = nowIso();
          sessions.set(sessionId, session);
        }

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
