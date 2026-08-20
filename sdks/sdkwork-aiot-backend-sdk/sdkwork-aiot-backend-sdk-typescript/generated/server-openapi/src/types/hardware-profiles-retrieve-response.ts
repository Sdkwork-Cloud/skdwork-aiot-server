import type { AiotHardwareProfileResponse } from './aiot-hardware-profile-response';

export interface HardwareProfilesRetrieveResponse {
  code: 0;
  data: unknown & { item: AiotHardwareProfileResponse; };
  /** Server-owned request correlation id. */
  traceId: string;
}
