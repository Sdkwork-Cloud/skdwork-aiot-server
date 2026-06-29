import type { AiotProtocolProfileResponse } from './aiot-protocol-profile-response';

export interface ProtocolProfilesCreateResponse201 {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
