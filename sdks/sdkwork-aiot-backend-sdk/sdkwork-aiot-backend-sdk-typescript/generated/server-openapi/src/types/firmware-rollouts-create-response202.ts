import type { AiotFirmwareRolloutResponse } from './aiot-firmware-rollout-response';

export interface FirmwareRolloutsCreateResponse202 {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
