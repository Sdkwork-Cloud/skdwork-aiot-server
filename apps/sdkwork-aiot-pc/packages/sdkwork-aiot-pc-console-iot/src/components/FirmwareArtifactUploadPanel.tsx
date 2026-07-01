import { useState } from 'react';
import { PackageOpen, UploadCloud } from 'lucide-react';
import { Button, StatusNotice } from '@sdkwork/ui-pc-react';
import type { SdkworkFirmwareService } from '../firmware-service';
import { createSdkworkFirmwareService } from '../firmware-service';

export interface SdkworkFirmwareArtifactUploadPanelProps {
  service?: SdkworkFirmwareService;
}

export function SdkworkFirmwareArtifactUploadPanel({
  service: serviceProp,
}: SdkworkFirmwareArtifactUploadPanelProps) {
  const service = serviceProp ?? createSdkworkFirmwareService();
  const [artifactKey, setArtifactKey] = useState('fw-main');
  const [version, setVersion] = useState('1.0.0');
  const [targetChipFamily, setTargetChipFamily] = useState('esp32');
  const [targetRuntimeProfile, setTargetRuntimeProfile] = useState('xiaozhi');
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [isUploading, setIsUploading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const handleUpload = async () => {
    if (!selectedFile) {
      setErrorMessage('Select a firmware binary before uploading.');
      return;
    }

    setIsUploading(true);
    setErrorMessage(null);
    setSuccessMessage(null);

    try {
      const result = await service.uploadArtifact({
        file: selectedFile,
        artifactKey,
        version,
        targetChipFamily: targetChipFamily.trim() || undefined,
        targetRuntimeProfile: targetRuntimeProfile.trim() || undefined,
      });
      setSuccessMessage(
        `Registered ${result.artifact.artifactKey} v${result.artifact.version} via Drive node ${result.nodeId}.`,
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : 'Firmware upload failed.');
    } finally {
      setIsUploading(false);
    }
  };

  return (
    <section className="rounded-[1.75rem] border border-zinc-200 bg-white p-5 shadow-sm">
      <div className="flex items-start gap-3">
        <div className="rounded-2xl bg-cyan-50 p-3 text-cyan-700">
          <PackageOpen className="h-5 w-5" />
        </div>
        <div className="min-w-0 flex-1">
          <h2 className="text-lg font-semibold text-zinc-900">Firmware OTA Artifacts</h2>
          <p className="mt-1 text-sm leading-6 text-zinc-600">
            Upload firmware binaries through SDKWork Drive, then register artifact metadata for OTA rollouts.
          </p>
        </div>
      </div>

      <div className="mt-5 grid gap-4 md:grid-cols-2">
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-zinc-700">Artifact key</span>
          <input
            className="w-full rounded-xl border border-zinc-200 px-3 py-2"
            onChange={(event) => setArtifactKey(event.target.value)}
            value={artifactKey}
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-zinc-700">Version</span>
          <input
            className="w-full rounded-xl border border-zinc-200 px-3 py-2"
            onChange={(event) => setVersion(event.target.value)}
            value={version}
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-zinc-700">Target chip family</span>
          <input
            className="w-full rounded-xl border border-zinc-200 px-3 py-2"
            onChange={(event) => setTargetChipFamily(event.target.value)}
            value={targetChipFamily}
          />
        </label>
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-zinc-700">Target runtime profile</span>
          <input
            className="w-full rounded-xl border border-zinc-200 px-3 py-2"
            onChange={(event) => setTargetRuntimeProfile(event.target.value)}
            value={targetRuntimeProfile}
          />
        </label>
      </div>

      <label className="mt-4 block text-sm">
        <span className="mb-1 block font-medium text-zinc-700">Firmware binary</span>
        <input
          accept=".bin,.zip,.tar,.gz,.img,application/octet-stream"
          className="block w-full text-sm text-zinc-600"
          onChange={(event) => setSelectedFile(event.target.files?.[0] ?? null)}
          type="file"
        />
      </label>

      <div className="mt-4 flex flex-wrap items-center gap-3">
        <Button disabled={isUploading} onClick={() => void handleUpload()} type="button">
          <UploadCloud className="mr-2 h-4 w-4" />
          {isUploading ? 'Uploading…' : 'Upload via Drive'}
        </Button>
        {selectedFile ? (
          <span className="text-sm text-zinc-500">
            {selectedFile.name} ({selectedFile.size} bytes)
          </span>
        ) : null}
      </div>

      {errorMessage ? (
        <div className="mt-4">
          <StatusNotice tone="danger">{errorMessage}</StatusNotice>
        </div>
      ) : null}
      {successMessage ? (
        <div className="mt-4">
          <StatusNotice tone="success">{successMessage}</StatusNotice>
        </div>
      ) : null}
    </section>
  );
}
