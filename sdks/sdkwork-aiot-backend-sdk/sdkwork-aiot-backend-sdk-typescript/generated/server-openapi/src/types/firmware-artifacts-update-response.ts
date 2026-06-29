import type { AiotFirmwareArtifactResponse } from './aiot-firmware-artifact-response';

export interface FirmwareArtifactsUpdateResponse {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
