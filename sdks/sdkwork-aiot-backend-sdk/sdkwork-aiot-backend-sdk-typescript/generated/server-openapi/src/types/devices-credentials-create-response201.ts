import type { AiotCredentialResponse } from './aiot-credential-response';

export interface DevicesCredentialsCreateResponse201 {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
