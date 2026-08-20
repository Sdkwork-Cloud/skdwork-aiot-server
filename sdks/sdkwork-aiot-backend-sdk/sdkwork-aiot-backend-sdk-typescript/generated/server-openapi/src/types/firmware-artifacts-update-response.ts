import type { AiotFirmwareArtifactResponse } from './aiot-firmware-artifact-response';

export interface FirmwareArtifactsUpdateResponse {
  code: 0;
  data: unknown & { item: AiotFirmwareArtifactResponse; };
  /** Server-owned request correlation id. */
  traceId: string;
}
