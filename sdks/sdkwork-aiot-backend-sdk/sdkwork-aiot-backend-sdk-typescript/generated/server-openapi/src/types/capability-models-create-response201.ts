import type { AiotCapabilityModelResponse } from './aiot-capability-model-response';

export interface CapabilityModelsCreateResponse201 {
  code: 0;
  data: unknown & Record<string, unknown>;
  /** Server-owned request correlation id. */
  traceId: string;
}
