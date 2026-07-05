export type SdkworkIotNodeKind = "gateway" | "sensor";
export type SdkworkIotNodeHealthLevel = "critical" | "healthy" | "warning";
export type SdkworkIotSensorStatus = "alert" | "nominal" | "offline";
export type SdkworkIotAlertSeverity = "critical" | "info" | "warning";
export type SdkworkIotSitePostureLevel = "degraded" | "secure" | "vulnerable";
export type SdkworkIotRouteSection = "alerts" | "fleet" | "overview" | "posture" | "remote-control";
export type SdkworkIotRemoteIntentId = "isolate-segment" | "run-diagnostics" | "sync-configuration";

export interface SdkworkIotCapabilityManifest {
  description: string;
  host?: string;
  id: string;
  packageNames: string[];
  theme?: string;
  title: string;
}

export interface SdkworkIotWorkspaceManifest extends SdkworkIotCapabilityManifest {
  capability: "iot";
  routePath: string;
}

export interface CreateIotWorkspaceManifestOptions
  extends Partial<Pick<SdkworkIotCapabilityManifest, "description" | "host" | "id" | "packageNames" | "theme" | "title">> {
  routePath?: string;
}

export interface SdkworkIotRouteIntent {
  alertId?: string;
  focusWindow: boolean;
  nodeId?: string;
  route: string;
  section?: SdkworkIotRouteSection;
  source: "iot-workspace";
  type: "iot-route-intent";
}

export interface CreateIotRouteIntentOptions {
  alertId?: string;
  basePath?: string;
  focusWindow?: boolean;
  nodeId?: string;
  section?: SdkworkIotRouteSection;
}

export interface SdkworkIotSensorSignal {
  id: string;
  status: SdkworkIotSensorStatus;
  title: string;
  unit: string;
  value: number;
}

export interface SdkworkIotNode {
  firmwareVersion: string;
  gatewayId?: string;
  healthLevel: SdkworkIotNodeHealthLevel;
  id: string;
  kind: SdkworkIotNodeKind;
  labels: string[];
  lastSeenAt: string;
  name: string;
  online: boolean;
  postureScore: number | null;
  route: string;
  sensors: SdkworkIotSensorSignal[];
  site: string;
}

export interface SdkworkIotAlert {
  acknowledged: boolean;
  createdAt: string;
  id: string;
  nodeId: string;
  route: string;
  severity: SdkworkIotAlertSeverity;
  title: string;
}

export interface SdkworkIotSitePosture {
  criticalNodes: number;
  level: SdkworkIotSitePostureLevel;
  nodeCount: number;
  postureScore: number | null;
  site: string;
  warningNodes: number;
}

export interface SdkworkIotRemoteControlIntent {
  description: string;
  id: SdkworkIotRemoteIntentId;
  label: string;
  routeIntent: SdkworkIotRouteIntent;
}

export interface SdkworkListPageInfo {
  hasMore: boolean;
  page: number;
  pageSize: number;
  total?: number;
}

export interface SdkworkIotFleetSummary {
  acknowledgedAlerts: number;
  criticalNodes: number;
  gatewayCount: number;
  healthyNodes: number;
  offlineNodes: number;
  onlineNodes: number;
  postureAverage: number;
  sensorCount: number;
  totalAlerts: number;
  totalNodes: number;
  unacknowledgedAlerts: number;
  warningNodes: number;
}

export interface SdkworkIotCatalogData {
  alerts: SdkworkIotAlert[];
  isAuthenticated: boolean;
  nodes: SdkworkIotNode[];
  pageInfo?: SdkworkListPageInfo;
  remoteControlIntents: SdkworkIotRemoteControlIntent[];
  routeIntents: {
    alerts: SdkworkIotRouteIntent;
    fleet: SdkworkIotRouteIntent;
    overview: SdkworkIotRouteIntent;
    posture: SdkworkIotRouteIntent;
    remoteControl: SdkworkIotRouteIntent;
  };
  selectedNodeId: string | null;
  sitePosture: SdkworkIotSitePosture[];
  summary: SdkworkIotFleetSummary;
}

export interface CreateEmptySdkworkIotCatalogOptions {
  alerts?: readonly SdkworkIotAlert[];
  basePath?: string;
  isAuthenticated?: boolean;
  nodes?: readonly SdkworkIotNode[];
  pageInfo?: SdkworkListPageInfo;
  selectedNodeId?: string | null;
}

export const iotPackageMeta = {
  architecture: "pc-console",
  domain: "iot",
  package: "@sdkwork/aiot-pc-console-iot",
  product: "sdkwork-aiot",
  status: "ready",
} as const;

export type IotPackageMeta = typeof iotPackageMeta;

function normalizeBasePath(basePath: string | undefined): string {
  const normalized = (basePath ?? "/iot").trim();
  if (!normalized || normalized === "/") {
    return "/iot";
  }

  return normalized.endsWith("/") ? normalized.slice(0, -1) : normalized;
}

function roundPercent(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.max(0, Math.min(100, Math.round(value)));
}

function toHealthRank(value: SdkworkIotNodeHealthLevel): number {
  if (value === "healthy") {
    return 0;
  }

  if (value === "warning") {
    return 1;
  }

  return 2;
}

function toSeverityRank(value: SdkworkIotAlertSeverity): number {
  if (value === "critical") {
    return 0;
  }

  if (value === "warning") {
    return 1;
  }

  return 2;
}

function createSdkworkIotCapabilityManifest(
  options: SdkworkIotCapabilityManifest,
): SdkworkIotCapabilityManifest {
  return {
    description: options.description,
    ...(options.host ? { host: options.host } : {}),
    id: options.id,
    packageNames: [...options.packageNames],
    ...(options.theme ? { theme: options.theme } : {}),
    title: options.title,
  };
}

export function createDefaultSdkworkIotNodes(): SdkworkIotNode[] {
  return [
    {
      firmwareVersion: "gw-3.4.2",
      healthLevel: "healthy",
      id: "node-gateway-shanghai",
      kind: "gateway",
      labels: ["hub", "primary"],
      lastSeenAt: "2026-04-03T13:20:00.000Z",
      name: "Shanghai Hub Gateway",
      online: true,
      postureScore: 95,
      route: "/iot/node-gateway-shanghai",
      sensors: [
        { id: "sensor-pressure-line-1", status: "nominal", title: "Pressure Line 1", unit: "kPa", value: 102 },
        { id: "sensor-temp-hub-core", status: "nominal", title: "Hub Core Temp", unit: "C", value: 28 },
      ],
      site: "Shanghai DC",
    },
    {
      firmwareVersion: "gw-3.2.8",
      healthLevel: "warning",
      id: "node-gateway-plant-east",
      kind: "gateway",
      labels: ["plant", "edge"],
      lastSeenAt: "2026-04-03T13:03:00.000Z",
      name: "Plant East Edge Gateway",
      online: true,
      postureScore: 74,
      route: "/iot/node-gateway-plant-east",
      sensors: [
        { id: "sensor-flow-pipe-4", status: "alert", title: "Flow Pipe 4", unit: "L/min", value: 42 },
      ],
      site: "Plant East",
    },
    {
      firmwareVersion: "sn-2.1.4",
      gatewayId: "node-gateway-plant-east",
      healthLevel: "healthy",
      id: "node-sensor-plant-east",
      kind: "sensor",
      labels: ["plant", "valve"],
      lastSeenAt: "2026-04-03T13:08:00.000Z",
      name: "Plant East Valve Sensor",
      online: true,
      postureScore: 87,
      route: "/iot/node-sensor-plant-east",
      sensors: [
        { id: "sensor-valve-e-07", status: "nominal", title: "Valve E-07", unit: "%", value: 62 },
      ],
      site: "Plant East",
    },
    {
      firmwareVersion: "sn-2.0.9",
      gatewayId: "node-gateway-plant-east",
      healthLevel: "critical",
      id: "node-sensor-plant-west",
      kind: "sensor",
      labels: ["plant", "valve"],
      lastSeenAt: "2026-04-03T09:31:00.000Z",
      name: "Plant West Valve Sensor",
      online: false,
      postureScore: 33,
      route: "/iot/node-sensor-plant-west",
      sensors: [
        { id: "sensor-valve-w-11", status: "offline", title: "Valve W-11", unit: "%", value: 0 },
      ],
      site: "Plant West",
    },
  ];
}

export function createDefaultSdkworkIotAlerts(): SdkworkIotAlert[] {
  return [
    {
      acknowledged: false,
      createdAt: "2026-04-03T13:04:00.000Z",
      id: "alert-flow-spike-east-1",
      nodeId: "node-gateway-plant-east",
      route: "/iot/alerts/alert-flow-spike-east-1",
      severity: "warning",
      title: "Flow spike detected near Pipe 4",
    },
    {
      acknowledged: false,
      createdAt: "2026-04-03T10:01:00.000Z",
      id: "alert-offline-west-sensor",
      nodeId: "node-sensor-plant-west",
      route: "/iot/alerts/alert-offline-west-sensor",
      severity: "critical",
      title: "Plant West Valve Sensor disconnected",
    },
    {
      acknowledged: true,
      createdAt: "2026-04-03T08:42:00.000Z",
      id: "alert-maintenance-window-sh",
      nodeId: "node-gateway-shanghai",
      route: "/iot/alerts/alert-maintenance-window-sh",
      severity: "info",
      title: "Scheduled firmware maintenance completed",
    },
  ];
}

function postureScoreValue(score: number | null): number {
  return score ?? 0;
}

export function sortSdkworkIotNodes(nodes: readonly SdkworkIotNode[]): SdkworkIotNode[] {
  return [...nodes].sort(
    (left, right) =>
      Number(right.kind === "gateway") - Number(left.kind === "gateway")
      || toHealthRank(left.healthLevel) - toHealthRank(right.healthLevel)
      || postureScoreValue(right.postureScore) - postureScoreValue(left.postureScore)
      || left.name.localeCompare(right.name),
  );
}

export function sortSdkworkIotAlerts(alerts: readonly SdkworkIotAlert[]): SdkworkIotAlert[] {
  return [...alerts].sort(
    (left, right) =>
      Number(left.acknowledged) - Number(right.acknowledged)
      || toSeverityRank(left.severity) - toSeverityRank(right.severity)
      || right.createdAt.localeCompare(left.createdAt),
  );
}

export function summarizeSdkworkIotFleet(
  nodes: readonly SdkworkIotNode[],
  alerts: readonly SdkworkIotAlert[] = [],
): SdkworkIotFleetSummary {
  const summary = nodes.reduce(
    (state, node) => {
      state.totalNodes += 1;
      if (node.postureScore !== null) {
        state.postureAverage += roundPercent(node.postureScore);
        state.scoredNodes += 1;
      }
      if (node.kind === "gateway") {
        state.gatewayCount += 1;
      } else {
        state.sensorCount += 1;
      }

      if (node.online) {
        state.onlineNodes += 1;
      } else {
        state.offlineNodes += 1;
      }

      if (node.healthLevel === "critical") {
        state.criticalNodes += 1;
      } else if (node.healthLevel === "warning") {
        state.warningNodes += 1;
      } else {
        state.healthyNodes += 1;
      }

      return state;
    },
    {
      criticalNodes: 0,
      gatewayCount: 0,
      healthyNodes: 0,
      offlineNodes: 0,
      onlineNodes: 0,
      postureAverage: 0,
      scoredNodes: 0,
      sensorCount: 0,
      totalNodes: 0,
      warningNodes: 0,
    },
  );

  const acknowledgedAlerts = alerts.filter((alert) => alert.acknowledged).length;
  const totalAlerts = alerts.length;

  return {
    ...summary,
    acknowledgedAlerts,
    postureAverage: summary.scoredNodes > 0
      ? roundPercent(summary.postureAverage / summary.scoredNodes)
      : 0,
    totalAlerts,
    unacknowledgedAlerts: totalAlerts - acknowledgedAlerts,
  };
}

function toPostureLevel(score: number): SdkworkIotSitePostureLevel {
  if (score < 55) {
    return "vulnerable";
  }

  if (score < 80) {
    return "degraded";
  }

  return "secure";
}

export function createSdkworkIotSitePosture(nodes: readonly SdkworkIotNode[]): SdkworkIotSitePosture[] {
  const map = new Map<string, SdkworkIotNode[]>();
  nodes.forEach((node) => {
    const bucket = map.get(node.site) ?? [];
    bucket.push(node);
    map.set(node.site, bucket);
  });

  return [...map.entries()]
    .map(([site, siteNodes]) => {
      const scoredNodes = siteNodes.filter((node) => node.postureScore !== null);
      const postureScore = scoredNodes.length > 0
        ? roundPercent(
            scoredNodes.reduce((sum, node) => sum + (node.postureScore ?? 0), 0) / scoredNodes.length,
          )
        : 0;

      return {
        criticalNodes: siteNodes.filter((node) => node.healthLevel === "critical").length,
        level: toPostureLevel(postureScore),
        nodeCount: siteNodes.length,
        postureScore,
        site,
        warningNodes: siteNodes.filter((node) => node.healthLevel === "warning").length,
      };
    })
    .sort((left, right) => left.site.localeCompare(right.site));
}

export function createIotWorkspaceManifest({
  description = "IoT operations center for fleet posture, gateway health, alert timeline, and remote control intents.",
  host,
  id = "sdkwork-aiot-iot",
  packageNames = [
    "@sdkwork/aiot-pc-console-iot",
  ],
  routePath = "/iot",
  theme,
  title = "IoT Operations Center",
}: CreateIotWorkspaceManifestOptions = {}): SdkworkIotWorkspaceManifest {
  return {
    ...createSdkworkIotCapabilityManifest({
      description,
      host,
      id,
      packageNames,
      theme,
      title,
    }),
    capability: "iot",
    routePath: normalizeBasePath(routePath),
  };
}

export function createIotRouteIntent(
  options: CreateIotRouteIntentOptions = {},
): SdkworkIotRouteIntent {
  const basePath = normalizeBasePath(options.basePath);
  const params = new URLSearchParams();

  if (options.section) {
    params.set("section", options.section);
  }

  if (options.nodeId) {
    params.set("nodeId", options.nodeId);
  }

  if (options.alertId) {
    params.set("alertId", options.alertId);
  }

  const suffix = params.toString() ? `?${params.toString()}` : "";

  return {
    ...(options.alertId ? { alertId: options.alertId } : {}),
    focusWindow: options.focusWindow !== false,
    ...(options.nodeId ? { nodeId: options.nodeId } : {}),
    route: `${basePath}${suffix}`,
    ...(options.section ? { section: options.section } : {}),
    source: "iot-workspace",
    type: "iot-route-intent",
  };
}

function resolveSelectedNodeId(
  nodes: readonly SdkworkIotNode[],
  selectedNodeId: string | null | undefined,
): string | null {
  if (selectedNodeId && nodes.some((node) => node.id === selectedNodeId)) {
    return selectedNodeId;
  }

  return nodes.find((node) => node.kind === "gateway")?.id ?? nodes[0]?.id ?? null;
}

function createRemoteControlIntents(basePath: string): SdkworkIotRemoteControlIntent[] {
  return [
    {
      description: "Synchronize gateway policy and sensor thresholds across all online sites.",
      id: "sync-configuration",
      label: "Sync configuration",
      routeIntent: createIotRouteIntent({ basePath, section: "remote-control" }),
    },
    {
      description: "Start diagnostics bundle collection for unstable gateways and linked sensors.",
      id: "run-diagnostics",
      label: "Run diagnostics",
      routeIntent: createIotRouteIntent({ basePath, section: "remote-control" }),
    },
    {
      description: "Isolate affected edge segment and block northbound telemetry from risky nodes.",
      id: "isolate-segment",
      label: "Isolate segment",
      routeIntent: createIotRouteIntent({ basePath, section: "remote-control" }),
    },
  ];
}

export function createEmptySdkworkIotCatalog(
  options: CreateEmptySdkworkIotCatalogOptions = {},
): SdkworkIotCatalogData {
  const nodes = sortSdkworkIotNodes(options.nodes ?? []);
  const alerts = sortSdkworkIotAlerts(options.alerts ?? []);
  const basePath = options.basePath ?? "/iot";
  const summary = summarizeSdkworkIotFleet(nodes, alerts);
  const pageTotal = options.pageInfo?.total;

  return {
    alerts,
    isAuthenticated: Boolean(options.isAuthenticated),
    nodes,
    ...(options.pageInfo ? { pageInfo: options.pageInfo } : {}),
    remoteControlIntents: createRemoteControlIntents(basePath),
    routeIntents: {
      alerts: createIotRouteIntent({ basePath, section: "alerts" }),
      fleet: createIotRouteIntent({ basePath, section: "fleet" }),
      overview: createIotRouteIntent({ basePath }),
      posture: createIotRouteIntent({ basePath, section: "posture" }),
      remoteControl: createIotRouteIntent({ basePath, section: "remote-control" }),
    },
    selectedNodeId: resolveSelectedNodeId(nodes, options.selectedNodeId),
    sitePosture: createSdkworkIotSitePosture(nodes),
    summary: typeof pageTotal === "number"
      ? { ...summary, totalNodes: pageTotal }
      : summary,
  };
}
