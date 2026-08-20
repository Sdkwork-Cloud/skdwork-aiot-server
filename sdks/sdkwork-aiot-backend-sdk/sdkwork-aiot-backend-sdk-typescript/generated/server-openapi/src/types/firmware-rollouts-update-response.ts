import type { AiotFirmwareRolloutResponse } from './aiot-firmware-rollout-response';

export interface FirmwareRolloutsUpdateResponse {
  code: 0;
  data: unknown & { item: AiotFirmwareRolloutResponse; };
  /** Server-owned request correlation id. */
  traceId: string;
}
