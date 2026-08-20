import type { JsonValue } from './json-value';

export interface DevicesTwinRetrieveResponse {
  code: 0;
  data: unknown & { item: JsonValue; };
  /** Server-owned request correlation id. */
  traceId: string;
}
