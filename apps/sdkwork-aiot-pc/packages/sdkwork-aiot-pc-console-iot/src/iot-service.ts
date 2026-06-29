import { readPcReactRuntimeSession } from "@sdkwork/core-pc-react";
import { getAiotAppSdkClient } from "@sdkwork/aiot-pc-core";
import type {
  AiotDevice,
  SdkworkAiotAppClient,
} from "@sdkwork/aiot-app-sdk";
import {
  normalizeText,
  readNumber,
  readRecord,
  readString,
} from "@sdkwork/aiot-app-core";
import {
  createEmptySdkworkIotCatalog,
  type SdkworkIotAlert,
  type SdkworkIotCatalogData,
  type SdkworkIotNode,
} from "./iot";

export interface GetSdkworkIotCatalogInput {
  nodeId?: string | null;
}

export interface CreateSdkworkIotServiceOptions {
  aiotClient?: SdkworkAiotAppClient;
  alerts?: readonly SdkworkIotAlert[];
  getSessionTokens?: () => {
    authToken?: string;
  };
  nodes?: readonly SdkworkIotNode[];
}

export interface SdkworkIotService {
  getCatalog(input?: GetSdkworkIotCatalogInput): Promise<SdkworkIotCatalogData>;
  getEmptyCatalog(input?: GetSdkworkIotCatalogInput): SdkworkIotCatalogData;
}

function normalizeHealth(status: string): SdkworkIotNode["healthLevel"] {
  const normalized = status.toLowerCase();
  if (normalized === "online" || normalized === "healthy") {
    return "healthy";
  }
  if (normalized === "warning" || normalized === "degraded") {
    return "warning";
  }
  return "critical";
}

function mapAiotDeviceToNode(device: AiotDevice): SdkworkIotNode {
  const metadata = readRecord(device.metadata);
  const deviceId = device.deviceId || device.id;
  const status = readString(device.status);
  const kind = readString(metadata.kind, readString(metadata.type, device.chipFamily ? "gateway" : "sensor"));
  const labels = Array.isArray(metadata.labels)
    ? metadata.labels.map((item) => readString(item)).filter(Boolean)
    : [];

  return {
    firmwareVersion: readString(metadata.firmwareVersion),
    gatewayId: readString(metadata.gatewayId) || undefined,
    healthLevel: normalizeHealth(status),
    id: deviceId,
    kind: kind === "gateway" ? "gateway" : "sensor",
    labels,
    lastSeenAt: device.lastSeenAt ?? "",
    name: device.displayName || deviceId,
    online: status.toLowerCase() === "online",
    postureScore: readNumber(metadata.postureScore, status.toLowerCase() === "online" ? 90 : 30),
    route: `/iot/${deviceId}`,
    sensors: [],
    site: readString(metadata.site, readString(metadata.location, "Unassigned")),
  };
}

async function loadSdkNodes(options: CreateSdkworkIotServiceOptions): Promise<SdkworkIotNode[] | undefined> {
  if (!options.aiotClient) {
    return undefined;
  }

  const page = await options.aiotClient.iot.devices.list();
  return Array.isArray(page.items)
    ? page.items.map((item) => mapAiotDeviceToNode(item as AiotDevice))
    : [];
}

export function createSdkworkIotService(
  options: CreateSdkworkIotServiceOptions = {},
): SdkworkIotService {
  const getSessionTokens = options.getSessionTokens ?? (() => readPcReactRuntimeSession());

  return {
    async getCatalog(input = {}) {
      const sdkNodes = await loadSdkNodes(options);
      return createEmptySdkworkIotCatalog({
        alerts: options.alerts,
        isAuthenticated: Boolean(normalizeText(getSessionTokens().authToken)),
        nodes: sdkNodes ?? options.nodes,
        selectedNodeId: input.nodeId ?? null,
      });
    },

    getEmptyCatalog(input = {}) {
      return createEmptySdkworkIotCatalog({
        alerts: options.alerts,
        isAuthenticated: Boolean(normalizeText(getSessionTokens().authToken)),
        nodes: options.nodes,
        selectedNodeId: input.nodeId ?? null,
      });
    },
  };
}

export const sdkworkIotService = createSdkworkIotService({
  aiotClient: getAiotAppSdkClient(),
});
