import type { AiotHardwareProfileResponse } from './aiot-hardware-profile-response';

export interface HardwareProfilesCreateResponse201 {
  code: 0;
  data: unknown & { item: AiotHardwareProfileResponse; };
  /** Server-owned request correlation id. */
  traceId: string;
}
