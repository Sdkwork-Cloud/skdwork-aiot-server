import type { JsonValue } from './json-value';

export interface DevicesTwinRetrieveResponse {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
