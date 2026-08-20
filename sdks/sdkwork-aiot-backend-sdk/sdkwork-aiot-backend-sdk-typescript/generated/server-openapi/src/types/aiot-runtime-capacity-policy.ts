export interface AiotRuntimeCapacityPolicy {
  nodeId: string;
  /** int64 serialized as string. */
  maxConnectionsPerNode: string;
  /** int64 serialized as string. */
  maxSessionsPerTenant: string;
  maxInflightPerDevice: number;
  sessionLeaseTtlSeconds: number;
  sessionLeaseRenewSeconds: number;
  outboxMaxAttempts: number;
  deadLetterAfterAttempts: number;
  backpressure: { warnLag: string; rejectLag: string; deadLetterLag: string; };
  orderedDeviceCommands?: boolean;
  idempotentIngest?: boolean;
}
