import type { AiotHardwareProfileResponse } from './aiot-hardware-profile-response';

export interface HardwareProfilesCreateResponse201 {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
