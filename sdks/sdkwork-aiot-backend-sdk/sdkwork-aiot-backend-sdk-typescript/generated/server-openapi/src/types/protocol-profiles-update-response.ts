import type { AiotProtocolProfileResponse } from './aiot-protocol-profile-response';

export interface ProtocolProfilesUpdateResponse {
  code: 0;
  data: unknown & { item: AiotProtocolProfileResponse; };
  /** Server-owned request correlation id. */
  traceId: string;
}
