export type AiotConversationRole = 'user' | 'assistant' | 'system' | 'tool';

export type AiotConversationMessageStatus = 'pending' | 'completed' | 'failed';

export interface AiotConversationMessage {
  content: string;
  createdAt: string;
  id: string;
  role: AiotConversationRole;
  sessionId: string;
  status: AiotConversationMessageStatus;
  toolName?: string;
}

export interface AiotConversationSession {
  agentsAgentId?: string;
  agentsSessionId?: string;
  createdAt: string;
  deviceId: string;
  id: string;
  title: string;
  updatedAt: string;
}

export interface AiotVoiceDevice {
  chipFamily?: string;
  deviceId: string;
  displayName: string;
  online: boolean;
  productId?: string;
  status: string;
}

export interface AiotAgentToolCall {
  arguments: Record<string, unknown>;
  createdAt: string;
  id: string;
  name: string;
  result?: string;
  status: 'pending' | 'completed' | 'failed';
}
