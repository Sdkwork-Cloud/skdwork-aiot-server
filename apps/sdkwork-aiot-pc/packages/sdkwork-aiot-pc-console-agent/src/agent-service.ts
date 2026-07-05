import { getAiotAppSdkClient } from '@sdkwork/aiot-pc-core';
import {
  createAiotAgentService,
  type AiotAgentService,
  type AiotConversationMessage,
  type AiotConversationSession,
} from '@sdkwork/aiot-app-core';

export interface SdkworkAgentCatalog {
  activeSession: AiotConversationSession | null;
  messages: AiotConversationMessage[];
  sessions: AiotConversationSession[];
}

export interface CreateSdkworkAgentServiceOptions {
  agentService?: AiotAgentService;
}

export interface SdkworkAgentServicePort {
  createSession(deviceId: string, title?: string): AiotConversationSession;
  getCatalog(): SdkworkAgentCatalog;
  selectSession(sessionId: string | null): void;
  sendMessage(text: string): Promise<AiotConversationMessage>;
}

export function createSdkworkAgentService(
  options: CreateSdkworkAgentServiceOptions = {},
): SdkworkAgentServicePort {
  const agentService = options.agentService ?? createAiotAgentService({
    aiotClient: getAiotAppSdkClient(),
  });

  let activeSessionId: string | null = null;

  return {
    createSession(deviceId, title) {
      const session = agentService.createSession(deviceId, title);
      activeSessionId = session.id;
      return session;
    },

    getCatalog() {
      const sessions = agentService.getSessions();
      const activeSession = sessions.find((session) => session.id === activeSessionId) ?? sessions[0] ?? null;
      if (activeSession && activeSessionId !== activeSession.id) {
        activeSessionId = activeSession.id;
      }

      return {
        activeSession,
        messages: activeSession ? agentService.getMessages(activeSession.id) : [],
        sessions,
      };
    },

    selectSession(sessionId) {
      activeSessionId = sessionId;
    },

    async sendMessage(text) {
      const catalog = this.getCatalog();
      const session = catalog.activeSession;
      if (!session?.deviceId) {
        throw new Error('请先选择设备后再发送智能体消息。');
      }

      return agentService.sendMessage({
        deviceId: session.deviceId,
        sessionId: session.id,
        text,
      });
    },
  };
}

export const sdkworkAgentService = createSdkworkAgentService();
