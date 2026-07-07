import {
  formatDatetime,
  now,
  parseBool,
  parseNumber,
  uuid,
} from '@sdkwork/utils';
import { isBlank, trim } from '@sdkwork/utils';

export function readRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

export function readString(value: unknown, fallback = ''): string {
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  return fallback;
}

export function readNumber(value: unknown, fallback = 0): number {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === 'string' && !isBlank(value)) {
    return parseNumber(value) ?? fallback;
  }
  return fallback;
}

export function readBoolean(value: unknown, fallback = false): boolean {
  if (typeof value === 'boolean') {
    return value;
  }
  if (typeof value === 'string') {
    return parseBool(value) ?? fallback;
  }
  return fallback;
}

export function normalizeText(value: string | undefined): string {
  return trim(value ?? '').toLowerCase();
}
