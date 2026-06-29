import { readPcReactRuntimeSession } from "@sdkwork/core-pc-react";
import { getAiotAppSdkClient } from "@sdkwork/aiot-pc-core";
import type {
  AiotDevice,
  SdkworkAiotAppClient,
} from "@sdkwork/aiot-app-sdk";
import {
  normalizeText,
  readBoolean,
  readNumber,
  readRecord,
  readString,
} from "@sdkwork/aiot-app-core";
import {
  createEmptySdkworkDeviceCatalog,
  type SdkworkDeviceCatalogData,
  type SdkworkDeviceCapability,
  type SdkworkDevicePeripheral,
  type SdkworkManagedDevice,
} from "./device";

export interface GetSdkworkDeviceCatalogInput {
  deviceId?: string | null;
}

export interface CreateSdkworkDeviceServiceOptions {
  aiotClient?: SdkworkAiotAppClient;
  devices?: readonly SdkworkManagedDevice[];
  getSessionTokens?: () => {
    authToken?: string;
  };
}

export interface SdkworkDeviceService {
  getCatalog(input?: GetSdkworkDeviceCatalogInput): Promise<SdkworkDeviceCatalogData>;
  getEmptyCatalog(input?: GetSdkworkDeviceCatalogInput): SdkworkDeviceCatalogData;
}

function normalizeHealth(status: string): SdkworkManagedDevice["healthLevel"] {
  const normalized = status.toLowerCase();
  if (normalized === "online" || normalized === "healthy") {
    return "healthy";
  }
  if (normalized === "warning" || normalized === "degraded") {
    return "warning";
  }
  return "critical";
}

function readLabels(value: unknown): string[] {
  return Array.isArray(value)
    ? value.map((item) => readString(item)).filter(Boolean)
    : [];
}

function readCapabilities(value: unknown): SdkworkDeviceCapability[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.map((item, index) => {
    const record = readRecord(item);
    const area = readString(record.area);
    const status = readString(record.status);
    return {
      area: area === "graphics" || area === "network" || area === "security" || area === "storage" ? area : "compute",
      id: readString(record.id, `capability-${index + 1}`),
      label: readString(record.label, readString(record.title, `Capability ${index + 1}`)),
      score: readNumber(record.score),
      status: status === "limited" || status === "missing" ? status : "available",
    };
  });
}

function readPeripherals(value: unknown): SdkworkDevicePeripheral[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.map((item, index) => {
    const record = readRecord(item);
    const type = readString(record.type);
    const driverState = readString(record.driverState);
    return {
      connected: readBoolean(record.connected),
      driverState: driverState === "blocked" || driverState === "update-required" ? driverState : "ready",
      healthLevel: normalizeHealth(readString(record.healthLevel, readString(record.status, "healthy"))),
      id: readString(record.id, `peripheral-${index + 1}`),
      title: readString(record.title, readString(record.name, `Peripheral ${index + 1}`)),
      type: type === "camera" || type === "display" || type === "input" || type === "storage" ? type : "audio",
    };
  });
}

function readBatteryPercent(value: unknown): number | null {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const battery = readNumber(value, Number.NaN);
  return Number.isFinite(battery) ? Math.max(0, Math.min(100, Math.round(battery))) : null;
}

function mapAiotDeviceToManagedDevice(device: AiotDevice): SdkworkManagedDevice {
  const metadata = readRecord(device.metadata);
  const deviceId = device.deviceId || device.id;
  const status = readString(device.status);
  const online = status.toLowerCase() === "online" || status.toLowerCase() === "healthy";

  return {
    batteryPercent: readBatteryPercent(metadata.batteryPercent),
    capabilities: readCapabilities(metadata.capabilities),
    healthLevel: normalizeHealth(status),
    hostname: readString(metadata.hostname, readString(metadata.hostName, device.clientId ?? deviceId)),
    id: deviceId,
    isPrimary: readBoolean(metadata.isPrimary, false),
    labels: readLabels(metadata.labels),
    lastSeenAt: device.lastSeenAt ?? "",
    name: device.displayName || deviceId,
    online,
    osName: readString(metadata.osName, readString(metadata.os, readString(metadata.platform, device.chipFamily ?? ""))),
    peripherals: readPeripherals(metadata.peripherals),
    postureScore: readNumber(metadata.postureScore, online ? 90 : 30),
    route: `/devices/${deviceId}`,
  };
}

async function loadSdkDevices(options: CreateSdkworkDeviceServiceOptions): Promise<SdkworkManagedDevice[] | undefined> {
  if (!options.aiotClient) {
    return undefined;
  }

  const page = await options.aiotClient.iot.devices.list();
  return Array.isArray(page.items)
    ? page.items.map((item) => mapAiotDeviceToManagedDevice(item as AiotDevice))
    : [];
}

export function createSdkworkDeviceService(
  options: CreateSdkworkDeviceServiceOptions = {},
): SdkworkDeviceService {
  const getSessionTokens = options.getSessionTokens ?? (() => readPcReactRuntimeSession());

  return {
    async getCatalog(input = {}) {
      const sdkDevices = await loadSdkDevices(options);
      return createEmptySdkworkDeviceCatalog({
        devices: sdkDevices ?? options.devices,
        isAuthenticated: Boolean(normalizeText(getSessionTokens().authToken)),
        selectedDeviceId: input.deviceId ?? null,
      });
    },

    getEmptyCatalog(input = {}) {
      return createEmptySdkworkDeviceCatalog({
        devices: options.devices,
        isAuthenticated: Boolean(normalizeText(getSessionTokens().authToken)),
        selectedDeviceId: input.deviceId ?? null,
      });
    },
  };
}

export const sdkworkDeviceService = createSdkworkDeviceService({
  aiotClient: getAiotAppSdkClient(),
});
