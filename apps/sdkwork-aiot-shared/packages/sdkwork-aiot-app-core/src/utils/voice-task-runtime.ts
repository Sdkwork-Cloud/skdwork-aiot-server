import { readRecord, readString, sleep } from './session';

export const TERMINAL_VOICE_TASK_STATUSES = new Set([
  'succeeded',
  'failed',
  'cancelled',
  'expired',
]);

export function extractSdkResourceRecord(response: unknown): Record<string, unknown> {
  const record = readRecord(response);
  const data = readRecord(record.data);
  return Object.keys(data).length > 0 ? data : record;
}

export function extractSdkItems(response: unknown): unknown[] {
  const record = extractSdkResourceRecord(response);
  if (Array.isArray(record.items)) {
    return record.items;
  }
  if (Array.isArray(response)) {
    return response;
  }
  return [];
}

export function readVoiceTaskId(payload: Record<string, unknown>): string | null {
  const directId = payload.id ?? payload.taskId;
  if (typeof directId === 'string' && directId.trim()) {
    return directId.trim();
  }
  if (typeof directId === 'number') {
    return String(directId);
  }
  return null;
}

export function readMediaResourceUrl(mediaResource: unknown): { url?: string; mimeType?: string } {
  const resource = readRecord(mediaResource);
  const urlCandidate =
    resource.url
    ?? resource.sourceUri
    ?? resource.playbackUrl
    ?? resource.downloadUrl;
  return {
    url: typeof urlCandidate === 'string' && urlCandidate.trim() ? urlCandidate : undefined,
    mimeType: typeof resource.mimeType === 'string' ? resource.mimeType : undefined,
  };
}

export function readAssistantMessageText(completion: Record<string, unknown>): string | null {
  const assistantRecord = readRecord(completion.assistantMessage)
    ?? readRecord(completion.assistant_message);
  const text = readString(assistantRecord.content);
  return text || null;
}

export function readTranscriptText(payload: Record<string, unknown>): string | null {
  const text = readString(payload.text)
    || readString(payload.transcript)
    || readString(payload.transcriptText);
  return text || null;
}

export interface PollVoiceTaskOptions<T> {
  intervalMs?: number;
  onPoll: (taskId: string) => Promise<{ status: string; errorMessage?: string }>;
  resolveResult: (taskId: string) => Promise<T | null>;
  timeoutMs?: number;
}

export async function pollVoiceTaskUntilTerminal<T>(
  taskId: string,
  options: PollVoiceTaskOptions<T>,
): Promise<T> {
  const intervalMs = options.intervalMs ?? 1_500;
  const timeoutMs = options.timeoutMs ?? 120_000;
  const startedAt = Date.now();

  while (Date.now() - startedAt < timeoutMs) {
    const task = await options.onPoll(taskId);
    const status = readString(task.status, 'queued');

    if (status === 'failed') {
      throw new Error(task.errorMessage?.trim() || 'Voice task failed.');
    }

    if (TERMINAL_VOICE_TASK_STATUSES.has(status)) {
      const result = await options.resolveResult(taskId);
      if (result !== null) {
        return result;
      }
      throw new Error(`Voice task ${taskId} completed without a usable result.`);
    }

    await sleep(intervalMs);
  }

  throw new Error(`Voice task timed out (taskId: ${taskId}).`);
}
