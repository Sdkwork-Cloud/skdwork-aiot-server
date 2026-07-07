export interface AiotCredentialResponse {
  credentialId: string;
  deviceId: string;
  credentialType: 'bearer_token' | 'hmac' | 'mtls_x509' | 'hardware_attestation';
  status: string;
  expiresAt?: string;
  createdAt: string;
  revokedAt?: string;
  /** One-time provisioning secret returned only from devices.credentials.create. */
  issuedSecret?: string;
}
