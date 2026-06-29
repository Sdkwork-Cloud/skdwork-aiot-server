import type { AiotHardwareProfileResponse } from './aiot-hardware-profile-response';

export interface HardwareProfilesRetrieveResponse {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
