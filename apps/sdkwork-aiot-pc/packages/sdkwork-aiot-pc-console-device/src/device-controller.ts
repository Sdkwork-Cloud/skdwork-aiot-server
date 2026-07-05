import {
  useMemo,
  useSyncExternalStore,
} from "react";
import type {
  SdkworkDeviceCapabilityArea,
  SdkworkDeviceCatalogData,
  SdkworkDeviceHealthLevel,
  SdkworkManagedDevice,
} from "./device";
import {
  createSdkworkDeviceService,
  type GetSdkworkDeviceCatalogInput,
  type SdkworkDeviceService,
} from "./device-service";

export interface SdkworkDeviceControllerState {
  activeArea: SdkworkDeviceCapabilityArea | "all";
  activeHealthLevel: SdkworkDeviceHealthLevel | "all";
  catalog: SdkworkDeviceCatalogData;
  isBootstrapped: boolean;
  isLoading: boolean;
  lastError?: string;
  listPage: number;
  selectedDevice: SdkworkManagedDevice | null;
  selectedDeviceId: string | null;
  visibleDevices: SdkworkManagedDevice[];
}

export interface SdkworkDeviceController {
  bootstrap(input?: GetSdkworkDeviceCatalogInput): Promise<SdkworkDeviceControllerState>;
  getState(): SdkworkDeviceControllerState;
  goToListPage(page: number): Promise<SdkworkDeviceControllerState>;
  refresh(input?: GetSdkworkDeviceCatalogInput): Promise<SdkworkDeviceControllerState>;
  selectDevice(deviceId: string | null): void;
  service: SdkworkDeviceService;
  setArea(area: SdkworkDeviceCapabilityArea | "all"): void;
  setHealthLevel(healthLevel: SdkworkDeviceHealthLevel | "all"): void;
  subscribe(listener: () => void): () => void;
}

export interface CreateSdkworkDeviceControllerOptions {
  initialState?: Partial<SdkworkDeviceControllerState>;
  service?: Partial<SdkworkDeviceService>;
}

function deriveVisibleDevices(
  catalog: SdkworkDeviceCatalogData,
  activeHealthLevel: SdkworkDeviceHealthLevel | "all",
  activeArea: SdkworkDeviceCapabilityArea | "all",
): SdkworkManagedDevice[] {
  return catalog.devices.filter((device) => {
    if (activeHealthLevel !== "all" && device.healthLevel !== activeHealthLevel) {
      return false;
    }

    if (activeArea !== "all" && !device.capabilities.some((capability) => capability.area === activeArea)) {
      return false;
    }

    return true;
  });
}

function resolveSelectedDeviceId(
  devices: readonly SdkworkManagedDevice[],
  selectedDeviceId: string | null,
): string | null {
  if (selectedDeviceId && devices.some((device) => device.id === selectedDeviceId)) {
    return selectedDeviceId;
  }

  return devices.find((device) => device.isPrimary)?.id ?? devices[0]?.id ?? null;
}

function normalizeState(state: SdkworkDeviceControllerState): SdkworkDeviceControllerState {
  const visibleDevices = deriveVisibleDevices(state.catalog, state.activeHealthLevel, state.activeArea);
  const selectedDeviceId = resolveSelectedDeviceId(visibleDevices, state.selectedDeviceId);

  return {
    ...state,
    selectedDevice: visibleDevices.find((device) => device.id === selectedDeviceId) ?? null,
    selectedDeviceId,
    visibleDevices,
  };
}

export function createSdkworkDeviceController(
  options: CreateSdkworkDeviceControllerOptions = {},
): SdkworkDeviceController {
  const service: SdkworkDeviceService = options.service
    ? {
        ...createSdkworkDeviceService(),
        ...options.service,
      }
    : createSdkworkDeviceService();
  const fallbackCatalog = service.getEmptyCatalog();
  const listeners = new Set<() => void>();
  let state = normalizeState({
    activeArea: "all",
    activeHealthLevel: "all",
    catalog: fallbackCatalog,
    isBootstrapped: false,
    isLoading: false,
    listPage: 1,
    selectedDevice: null,
    selectedDeviceId: fallbackCatalog.selectedDeviceId,
    visibleDevices: fallbackCatalog.devices,
    ...options.initialState,
  });

  function emit(): void {
    listeners.forEach((listener) => listener());
  }

  function setState(
    next:
      | Partial<SdkworkDeviceControllerState>
      | ((currentState: SdkworkDeviceControllerState) => Partial<SdkworkDeviceControllerState>),
  ): void {
    const partial = typeof next === "function" ? next(state) : next;
    state = normalizeState({
      ...state,
      ...partial,
    });
    emit();
  }

  return {
    async bootstrap(input) {
      setState({
        isLoading: true,
        lastError: undefined,
      });

      try {
        const catalog = await service.getCatalog({
          ...input,
          page: input?.page ?? state.listPage,
        });
        setState({
          catalog,
          isBootstrapped: true,
          isLoading: false,
          listPage: catalog.pageInfo?.page ?? state.listPage,
          selectedDeviceId: catalog.selectedDeviceId,
        });
        return state;
      } catch (error) {
        setState({
          isLoading: false,
          lastError: error instanceof Error ? error.message : "Failed to load device center.",
        });
        throw error;
      }
    },

    async goToListPage(page) {
      const nextPage = Math.max(1, page);
      setState({
        isLoading: true,
        lastError: undefined,
        listPage: nextPage,
      });

      try {
        const catalog = await service.getCatalog({
          deviceId: state.selectedDeviceId,
          page: nextPage,
        });
        setState({
          catalog,
          isBootstrapped: true,
          isLoading: false,
          listPage: catalog.pageInfo?.page ?? nextPage,
          selectedDeviceId: catalog.selectedDeviceId,
        });
        return state;
      } catch (error) {
        setState({
          isLoading: false,
          lastError: error instanceof Error ? error.message : "Failed to load device page.",
        });
        throw error;
      }
    },

    getState() {
      return state;
    },

    async refresh(input) {
      const catalog = await service.getCatalog({
        ...input,
        deviceId: state.selectedDeviceId,
        page: input?.page ?? state.listPage,
      });
      setState({
        catalog,
        isBootstrapped: true,
        isLoading: false,
        listPage: catalog.pageInfo?.page ?? state.listPage,
      });
      return state;
    },

    selectDevice(deviceId) {
      setState({
        selectedDeviceId: deviceId,
      });
    },

    service,

    setArea(area) {
      setState({
        activeArea: area,
      });
    },

    setHealthLevel(healthLevel) {
      setState({
        activeHealthLevel: healthLevel,
      });
    },

    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

export function useSdkworkDeviceController(
  controller?: SdkworkDeviceController,
  service?: Partial<SdkworkDeviceService>,
): SdkworkDeviceController {
  return useMemo(
    () => controller ?? createSdkworkDeviceController(service ? { service } : undefined),
    [controller, service],
  );
}

export function useSdkworkDeviceControllerState(
  controller: SdkworkDeviceController,
): SdkworkDeviceControllerState {
  return useSyncExternalStore(
    controller.subscribe,
    controller.getState,
    controller.getState,
  );
}
