export type SdkworkDeviceCapabilityArea = "compute" | "graphics" | "network" | "security" | "storage";
export type SdkworkDeviceCapabilityStatus = "available" | "limited" | "missing";
export type SdkworkDeviceHealthLevel = "critical" | "healthy" | "warning";
export type SdkworkDevicePeripheralType = "audio" | "camera" | "display" | "input" | "storage";
export type SdkworkDeviceDriverState = "blocked" | "ready" | "update-required";
export type SdkworkDeviceRouteSection = "overview" | "peripherals" | "posture";

export interface SdkworkDeviceCapabilityManifest {
  description: string;
  host?: string;
  id: string;
  packageNames: string[];
  theme?: string;
  title: string;
}

export interface SdkworkDeviceWorkspaceManifest extends SdkworkDeviceCapabilityManifest {
  capability: "device";
  routePath: string;
}

export interface CreateDeviceWorkspaceManifestOptions
  extends Partial<
    Pick<SdkworkDeviceCapabilityManifest, "description" | "host" | "id" | "packageNames" | "theme" | "title">
  > {
  routePath?: string;
}

export interface SdkworkDeviceRouteIntent {
  deviceId?: string;
  focusWindow: boolean;
  route: string;
  section?: SdkworkDeviceRouteSection;
  source: "device-workspace";
  type: "device-route-intent";
}

export interface CreateDeviceRouteIntentOptions {
  basePath?: string;
  deviceId?: string;
  focusWindow?: boolean;
  section?: SdkworkDeviceRouteSection;
}

export interface SdkworkDeviceCapability {
  area: SdkworkDeviceCapabilityArea;
  id: string;
  label: string;
  score: number;
  status: SdkworkDeviceCapabilityStatus;
}

export interface SdkworkDevicePeripheral {
  connected: boolean;
  driverState: SdkworkDeviceDriverState;
  healthLevel: SdkworkDeviceHealthLevel;
  id: string;
  title: string;
  type: SdkworkDevicePeripheralType;
}

export interface SdkworkManagedDevice {
  batteryPercent: number | null;
  capabilities: SdkworkDeviceCapability[];
  healthLevel: SdkworkDeviceHealthLevel;
  hostname: string;
  id: string;
  isPrimary: boolean;
  labels: string[];
  lastSeenAt: string;
  name: string;
  online: boolean;
  osName: string;
  peripherals: SdkworkDevicePeripheral[];
  postureScore: number | null;
  route: string;
}

export interface SdkworkListPageInfo {
  hasMore: boolean;
  page: number;
  pageSize: number;
  total?: number;
}

export interface SdkworkDeviceSummary {
  connectedPeripherals: number;
  criticalDevices: number;
  healthyDevices: number;
  postureAverage: number;
  primaryDeviceId: string | null;
  totalDevices: number;
  warningDevices: number;
}

export interface SdkworkDeviceCatalogData {
  devices: SdkworkManagedDevice[];
  isAuthenticated: boolean;
  pageInfo?: SdkworkListPageInfo;
  routeIntents: {
    overview: SdkworkDeviceRouteIntent;
    peripherals: SdkworkDeviceRouteIntent;
    posture: SdkworkDeviceRouteIntent;
  };
  selectedDeviceId: string | null;
  summary: SdkworkDeviceSummary;
}

export interface CreateEmptySdkworkDeviceCatalogOptions {
  basePath?: string;
  devices?: readonly SdkworkManagedDevice[];
  isAuthenticated?: boolean;
  pageInfo?: SdkworkListPageInfo;
  selectedDeviceId?: string | null;
}

export const devicePackageMeta = {
  architecture: "pc-console",
  domain: "device",
  package: "@sdkwork/aiot-pc-console-device",
  product: "sdkwork-aiot",
  status: "ready",
} as const;

export type DevicePackageMeta = typeof devicePackageMeta;

function normalizeBasePath(basePath: string | undefined): string {
  const normalized = (basePath ?? "/devices").trim();
  if (!normalized || normalized === "/") {
    return "/devices";
  }

  return normalized.endsWith("/") ? normalized.slice(0, -1) : normalized;
}

function clampScore(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.max(0, Math.min(100, Math.round(value)));
}

function toHealthRank(value: SdkworkDeviceHealthLevel): number {
  if (value === "critical") {
    return 0;
  }

  if (value === "warning") {
    return 1;
  }

  return 2;
}

function createSdkworkDeviceCapabilityManifest(
  options: SdkworkDeviceCapabilityManifest,
): SdkworkDeviceCapabilityManifest {
  return {
    description: options.description,
    ...(options.host ? { host: options.host } : {}),
    id: options.id,
    packageNames: [...options.packageNames],
    ...(options.theme ? { theme: options.theme } : {}),
    title: options.title,
  };
}

export function createDefaultSdkworkManagedDevices(): SdkworkManagedDevice[] {
  return [
    {
      batteryPercent: null,
      capabilities: [
        { area: "compute", id: "compute-cpu", label: "CPU budget", score: 94, status: "available" },
        { area: "graphics", id: "graphics-gpu", label: "GPU acceleration", score: 92, status: "available" },
        { area: "network", id: "network-vpn", label: "VPN posture", score: 88, status: "available" },
        { area: "security", id: "security-tpm", label: "Trusted module", score: 96, status: "available" },
      ],
      healthLevel: "healthy",
      hostname: "studio-ws-01",
      id: "device-studio-workstation",
      isPrimary: true,
      labels: ["desktop", "design", "primary"],
      lastSeenAt: "2026-04-03T11:20:00.000Z",
      name: "Studio Workstation",
      online: true,
      osName: "Windows 11 Pro",
      peripherals: [
        { connected: true, driverState: "ready", healthLevel: "healthy", id: "peripheral-cam", title: "4K Camera", type: "camera" },
        { connected: true, driverState: "ready", healthLevel: "healthy", id: "peripheral-audio", title: "Audio Interface", type: "audio" },
      ],
      postureScore: 93,
      route: "/devices/device-studio-workstation",
    },
    {
      batteryPercent: 72,
      capabilities: [
        { area: "compute", id: "compute-mobile", label: "Mobile compute", score: 78, status: "available" },
        { area: "network", id: "network-wifi", label: "WiFi roaming", score: 68, status: "limited" },
        { area: "storage", id: "storage-local", label: "Local cache", score: 74, status: "available" },
        { area: "security", id: "security-biometric", label: "Biometric login", score: 82, status: "available" },
      ],
      healthLevel: "warning",
      hostname: "field-laptop-02",
      id: "device-field-laptop",
      isPrimary: false,
      labels: ["laptop", "field"],
      lastSeenAt: "2026-04-03T10:55:00.000Z",
      name: "Field Laptop",
      online: true,
      osName: "Windows 11 Pro",
      peripherals: [
        { connected: true, driverState: "update-required", healthLevel: "warning", id: "peripheral-display", title: "Portable Display", type: "display" },
      ],
      postureScore: 76,
      route: "/devices/device-field-laptop",
    },
    {
      batteryPercent: null,
      capabilities: [
        { area: "compute", id: "compute-render", label: "Render agent", score: 64, status: "limited" },
        { area: "graphics", id: "graphics-remote", label: "Remote GPU", score: 58, status: "limited" },
        { area: "network", id: "network-lan", label: "LAN latency", score: 41, status: "limited" },
        { area: "security", id: "security-agent", label: "Security agent", score: 32, status: "missing" },
      ],
      healthLevel: "critical",
      hostname: "render-node-07",
      id: "device-render-node",
      isPrimary: false,
      labels: ["render", "node"],
      lastSeenAt: "2026-04-03T08:15:00.000Z",
      name: "Render Node",
      online: false,
      osName: "Windows Server",
      peripherals: [
        { connected: false, driverState: "blocked", healthLevel: "critical", id: "peripheral-storage", title: "Scratch Array", type: "storage" },
      ],
      postureScore: 39,
      route: "/devices/device-render-node",
    },
  ];
}

export function sortSdkworkManagedDevices(
  devices: readonly SdkworkManagedDevice[],
): SdkworkManagedDevice[] {
  return [...devices].sort(
    (left, right) =>
      Number(right.isPrimary) - Number(left.isPrimary)
      || toHealthRank(left.healthLevel) - toHealthRank(right.healthLevel)
      || (right.postureScore ?? 0) - (left.postureScore ?? 0)
      || left.name.localeCompare(right.name),
  );
}

type SdkworkDeviceSummaryAccumulator = SdkworkDeviceSummary & {
  scoredDevices: number;
};

export function summarizeSdkworkDevices(
  devices: readonly SdkworkManagedDevice[],
): SdkworkDeviceSummary {
  const summary = devices.reduce<SdkworkDeviceSummaryAccumulator>(
    (state, device) => {
      state.totalDevices += 1;
      state.connectedPeripherals += device.peripherals.filter((peripheral) => peripheral.connected).length;
      if (device.postureScore !== null) {
        state.postureAverage += clampScore(device.postureScore);
        state.scoredDevices += 1;
      }

      if (device.healthLevel === "critical") {
        state.criticalDevices += 1;
      } else if (device.healthLevel === "warning") {
        state.warningDevices += 1;
      } else {
        state.healthyDevices += 1;
      }

      if (device.isPrimary) {
        state.primaryDeviceId = device.id;
      }

      return state;
    },
    {
      connectedPeripherals: 0,
      criticalDevices: 0,
      healthyDevices: 0,
      postureAverage: 0,
      primaryDeviceId: null,
      scoredDevices: 0,
      totalDevices: 0,
      warningDevices: 0,
    },
  );

  return {
    connectedPeripherals: summary.connectedPeripherals,
    criticalDevices: summary.criticalDevices,
    healthyDevices: summary.healthyDevices,
    postureAverage: summary.scoredDevices > 0
      ? clampScore(summary.postureAverage / summary.scoredDevices)
      : 0,
    primaryDeviceId: summary.primaryDeviceId,
    totalDevices: summary.totalDevices,
    warningDevices: summary.warningDevices,
  };
}

export function createDeviceWorkspaceManifest({
  description = "Device center for local machine posture, capability coverage, and managed peripheral visibility.",
  host,
  id = "sdkwork-aiot-device",
  packageNames = [
    "@sdkwork/aiot-pc-console-device",
  ],
  routePath = "/devices",
  theme,
  title = "Device Center",
}: CreateDeviceWorkspaceManifestOptions = {}): SdkworkDeviceWorkspaceManifest {
  return {
    ...createSdkworkDeviceCapabilityManifest({
      description,
      host,
      id,
      packageNames,
      theme,
      title,
    }),
    capability: "device",
    routePath: normalizeBasePath(routePath),
  };
}

export function createDeviceRouteIntent(
  options: CreateDeviceRouteIntentOptions = {},
): SdkworkDeviceRouteIntent {
  const basePath = normalizeBasePath(options.basePath);
  const params = new URLSearchParams();

  if (options.section) {
    params.set("section", options.section);
  }

  if (options.deviceId) {
    params.set("deviceId", options.deviceId);
  }

  const suffix = params.toString() ? `?${params.toString()}` : "";

  return {
    ...(options.deviceId ? { deviceId: options.deviceId } : {}),
    focusWindow: options.focusWindow !== false,
    route: `${basePath}${suffix}`,
    ...(options.section ? { section: options.section } : {}),
    source: "device-workspace",
    type: "device-route-intent",
  };
}

function resolveSelectedDeviceId(
  devices: readonly SdkworkManagedDevice[],
  selectedDeviceId: string | null | undefined,
): string | null {
  if (selectedDeviceId && devices.some((device) => device.id === selectedDeviceId)) {
    return selectedDeviceId;
  }

  return devices.find((device) => device.isPrimary)?.id ?? devices[0]?.id ?? null;
}

export function createEmptySdkworkDeviceCatalog(
  options: CreateEmptySdkworkDeviceCatalogOptions = {},
): SdkworkDeviceCatalogData {
  const devices = sortSdkworkManagedDevices(options.devices ?? []);
  const basePath = options.basePath ?? "/devices";
  const summary = summarizeSdkworkDevices(devices);
  const pageTotal = options.pageInfo?.total;

  return {
    devices,
    isAuthenticated: Boolean(options.isAuthenticated),
    ...(options.pageInfo ? { pageInfo: options.pageInfo } : {}),
    routeIntents: {
      overview: createDeviceRouteIntent({ basePath }),
      peripherals: createDeviceRouteIntent({ basePath, section: "peripherals" }),
      posture: createDeviceRouteIntent({ basePath, section: "posture" }),
    },
    selectedDeviceId: resolveSelectedDeviceId(devices, options.selectedDeviceId),
    summary: typeof pageTotal === "number"
      ? { ...summary, totalDevices: pageTotal }
      : summary,
  };
}
