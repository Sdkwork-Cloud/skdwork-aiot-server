import type { AiotRuntimeCapacityPolicyResponse } from './aiot-runtime-capacity-policy-response';

export interface RuntimeCapacityRetrieveResponse {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
