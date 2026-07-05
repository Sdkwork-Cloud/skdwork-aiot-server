/** Optional upstream dialogue ports supplied by host apps (PC/H5) via sdkwork-agents / sdkwork-voice SDKs. */

export interface AiotAgentsDialoguePort {
  readonly configured: boolean;
  resolveAgentId(deviceId: string): string;
  createRemoteSession(agentId: string, title?: string): Promise<string>;
  sendChat(input: {
    agentId: string;
    remoteSessionId: string;
    text: string;
  }): Promise<string>;
}

export interface AiotVoiceSynthesisResult {
  audioUrl?: string;
  mimeType?: string;
  taskId?: string;
}

export interface AiotVoiceTranscriptionResult {
  taskId?: string;
  text: string;
}

export interface AiotVoiceDialoguePort {
  readonly configured: boolean;
  synthesize(text: string, options?: { model?: string; voice?: string }): Promise<AiotVoiceSynthesisResult>;
  transcribe?(input: {
    audioBlob: Blob;
    fileName?: string;
    language?: string;
    model?: string;
  }): Promise<AiotVoiceTranscriptionResult>;
}
