import type { AiotDevice } from './aiot-device';

export interface DevicesRetrieveResponse {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
