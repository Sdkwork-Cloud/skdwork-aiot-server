import { describe, expect, it, vi } from "vitest";
import * as iotModule from "../src";

describe("sdkwork-aiot-pc-console-iot service", () => {
  it("returns an empty catalog by default instead of seeded demo devices", () => {
    const createSdkworkIotService = (iotModule as Record<string, any>).createSdkworkIotService;

    const service = createSdkworkIotService({
      getSessionTokens: () => ({
        authToken: "",
      }),
    });

    expect(service.getEmptyCatalog()).toMatchObject({
      alerts: [],
      nodes: [],
      selectedNodeId: null,
      summary: {
        totalNodes: 0,
        totalAlerts: 0,
      },
    });
  });

  it("loads catalog devices through sdkwork-aiot-app-sdk when an AIoT client is provided", async () => {
    const createSdkworkIotService = (iotModule as Record<string, any>).createSdkworkIotService;
    const list = vi.fn().mockResolvedValue({
      code: "2000",
      data: [
        {
          id: "42",
          tenantId: "100001",
          organizationId: "org-1",
          deviceId: "gateway-42",
          displayName: "Gateway 42",
          productId: "gateway",
          chipFamily: "esp32_s3",
          status: "online",
          metadata: {
            kind: "gateway",
            labels: ["edge"],
            firmwareVersion: "gw-4.1.0",
            postureScore: 91,
            site: "Plant A",
          },
          lastSeenAt: "2026-06-06T08:30:00.000Z",
        },
      ],
    });

    const service = createSdkworkIotService({
      aiotClient: {
        iot: {
          devicesList: list,
        },
      },
      getSessionTokens: () => ({
        authToken: "token",
      }),
    });

    const catalog = await service.getCatalog({ nodeId: "gateway-42" });

    expect(list).toHaveBeenCalledWith();
    expect(catalog).toMatchObject({
      isAuthenticated: true,
      selectedNodeId: "gateway-42",
      nodes: [
        {
          id: "gateway-42",
          name: "Gateway 42",
          kind: "gateway",
          firmwareVersion: "gw-4.1.0",
          healthLevel: "healthy",
          online: true,
          postureScore: 91,
          site: "Plant A",
        },
      ],
      summary: {
        gatewayCount: 1,
        onlineNodes: 1,
        totalNodes: 1,
      },
    });
  });

  it("marks catalog authentication state from runtime session tokens and keeps selected node", async () => {
    const createDefaultSdkworkIotNodes = (iotModule as Record<string, any>).createDefaultSdkworkIotNodes;
    const createSdkworkIotService = (iotModule as Record<string, any>).createSdkworkIotService;

    const nodes = createDefaultSdkworkIotNodes();
    const authenticatedService = createSdkworkIotService({
      getSessionTokens: () => ({
        authToken: " fleet-token ",
      }),
      nodes,
    });

    const authenticatedCatalog = await authenticatedService.getCatalog({
      nodeId: "node-sensor-plant-east",
    });
    expect(authenticatedCatalog.isAuthenticated).toBe(true);
    expect(authenticatedCatalog.selectedNodeId).toBe("node-sensor-plant-east");

    const anonymousService = createSdkworkIotService({
      getSessionTokens: () => ({
        authToken: "   ",
      }),
      nodes,
    });

    expect(
      anonymousService.getEmptyCatalog({
        nodeId: "missing-node",
      }),
    ).toMatchObject({
      isAuthenticated: false,
      selectedNodeId: "node-gateway-shanghai",
      summary: {
        totalNodes: 4,
      },
    });
  });
});
