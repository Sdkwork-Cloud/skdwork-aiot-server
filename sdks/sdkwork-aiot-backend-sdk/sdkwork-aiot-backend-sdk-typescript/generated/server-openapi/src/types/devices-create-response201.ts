import type { StandardResourceResponse } from './standard-resource-response';

export interface DevicesCreateResponse201 {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
