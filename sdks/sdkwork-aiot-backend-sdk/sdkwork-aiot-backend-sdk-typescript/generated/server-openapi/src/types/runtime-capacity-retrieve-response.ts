import type { AiotRuntimeCapacityPolicyResponse } from './aiot-runtime-capacity-policy-response';

export interface RuntimeCapacityRetrieveResponse {
  code: 0;
  data: unknown & { item: AiotRuntimeCapacityPolicyResponse; };
  /** Server-owned request correlation id. */
  traceId: string;
}
