import type { AiotProtocolProfileResponse } from './aiot-protocol-profile-response';

export interface ProtocolProfilesRetrieveResponse {
  code: 0;
  data: unknown & { item: AiotProtocolProfileResponse; };
  /** Server-owned request correlation id. */
  traceId: string;
}
