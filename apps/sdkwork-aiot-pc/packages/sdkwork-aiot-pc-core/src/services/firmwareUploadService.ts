import type {
  AiotFirmwareArtifact,
  MediaResource as BackendMediaResource,
} from '@sdkwork/aiot-backend-sdk';
import { isBlank } from '@sdkwork/utils';

import { getAiotBackendSdkClient } from '../sdk/aiotBackendSdkClient';
import { getDriveAppSdkClient } from '../sdk/driveAppSdkClient';

const AIOT_FIRMWARE_APP_RESOURCE_TYPE = 'aiot-firmware-artifact';
const AIOT_FIRMWARE_UPLOAD_SCENE = 'aiot_firmware_upload';
const AIOT_FIRMWARE_UPLOAD_SOURCE = 'aiot_pc_admin';

export interface UploadAiotFirmwareArtifactInput {
  file: File;
  artifactKey: string;
  version: string;
  targetChipFamily?: string;
  targetRuntimeProfile?: string;
  signal?: AbortSignal;
}

export interface UploadAiotFirmwareArtifactResult {
  artifact: AiotFirmwareArtifact;
  mediaResource: BackendMediaResource;
  nodeId: string;
}

function buildUploaderFingerprint(file: File, sha256Hex: string): string {
  const contentType = file.type.trim() || 'application/octet-stream';
  return `sha256:${sha256Hex}:name:${file.name}:size:${file.size}:type:${contentType}`;
}

export async function sha256HexFromFile(file: File): Promise<string> {
  const buffer = await file.arrayBuffer();
  const digest = await crypto.subtle.digest('SHA-256', buffer);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
}

function buildDriveBackedFirmwareMediaResource(
  nodeId: string,
  file: File,
  checksumSha256?: string,
): BackendMediaResource {
  return {
    id: nodeId,
    kind: 'archive',
    source: 'drive',
    uri: `drive://nodes/${nodeId}`,
    fileName: file.name,
    mimeType: file.type.trim() || 'application/octet-stream',
    sizeBytes: String(file.size),
    checksum: checksumSha256
      ? { algorithm: 'sha256', value: checksumSha256 }
      : undefined,
  };
}

export async function uploadAiotFirmwareArtifactToDrive(
  input: UploadAiotFirmwareArtifactInput,
): Promise<UploadAiotFirmwareArtifactResult> {
  const artifactKey = input.artifactKey.trim();
  const version = input.version.trim();
  if (isBlank(artifactKey)) {
    throw new Error('artifactKey is required');
  }
  if (isBlank(version)) {
    throw new Error('version is required');
  }

  const sha256 = await sha256HexFromFile(input.file);
  const driveClient = getDriveAppSdkClient();
  const uploadResult = await driveClient.uploader.uploadArchive({
    file: input.file,
    appResourceType: AIOT_FIRMWARE_APP_RESOURCE_TYPE,
    appResourceId: artifactKey,
    scene: AIOT_FIRMWARE_UPLOAD_SCENE,
    source: AIOT_FIRMWARE_UPLOAD_SOURCE,
    fileFingerprint: buildUploaderFingerprint(input.file, sha256),
    originalFileName: input.file.name,
    contentType: input.file.type.trim() || 'application/octet-stream',
    signal: input.signal,
  });

  const nodeId = uploadResult.uploadItem.nodeId;
  const checksumSha256 = uploadResult.uploadItem.checksumSha256Hex ?? sha256;
  const mediaResource = buildDriveBackedFirmwareMediaResource(
    nodeId,
    input.file,
    checksumSha256,
  );

  const backendClient = getAiotBackendSdkClient();
  const created = await backendClient.iot.firmwareArtifacts.create({
    artifactKey,
    version,
    resource: mediaResource,
    sha256,
    targetChipFamily: input.targetChipFamily,
    targetRuntimeProfile: input.targetRuntimeProfile,
  });
  const artifact = (
    created && typeof created === 'object' && 'data' in created && created.data
      ? created.data
      : created
  ) as AiotFirmwareArtifact;

  return {
    artifact,
    mediaResource,
    nodeId,
  };
}
