import { coalesce, defaultIfBlank } from '@sdkwork/utils';
import { isBlank, trim } from '@sdkwork/utils';

type RuntimeImportMetaEnv = Record<string, string | boolean | undefined>;

export function readTrimmedString(value: unknown): string | undefined {
  if (typeof value !== 'string' || isBlank(value)) {
    return undefined;
  }
  return trim(value);
}

export function readImportMetaEnv(key: string): string | undefined {
  const env = (import.meta as ImportMeta & { env?: RuntimeImportMetaEnv }).env ?? {};
  return readTrimmedString(env[key]);
}

export function readProcessEnv(key: string): string | undefined {
  const processLike = (
    globalThis as typeof globalThis & {
      process?: {
        env?: Record<string, string | undefined>;
      };
    }
  ).process;
  return readTrimmedString(processLike?.env?.[key]);
}

export function readImportMetaEnvWithDefault(key: string, fallback: string): string {
  return defaultIfBlank(readImportMetaEnv(key), fallback);
}

export function readOptionalBearerToken(value: string | undefined): string | undefined {
  return coalesce(value);
}

export function readFirstNonBlank(
  values: Array<string | null | undefined>,
): string | undefined {
  return coalesce(...values);
}
