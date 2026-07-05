import {
  useMemo,
  useSyncExternalStore,
} from "react";
import type {
  SdkworkIotAlert,
  SdkworkIotAlertSeverity,
  SdkworkIotCatalogData,
  SdkworkIotNode,
  SdkworkIotNodeHealthLevel,
  SdkworkIotNodeKind,
} from "./iot";
import {
  createSdkworkIotService,
  type GetSdkworkIotCatalogInput,
  type SdkworkIotService,
} from "./iot-service";

export interface SdkworkIotControllerState {
  activeAlertSeverity: SdkworkIotAlertSeverity | "all";
  activeHealthLevel: SdkworkIotNodeHealthLevel | "all";
  activeKind: SdkworkIotNodeKind | "all";
  activeSite: string | "all";
  catalog: SdkworkIotCatalogData;
  isBootstrapped: boolean;
  isLoading: boolean;
  lastError?: string;
  listPage: number;
  selectedNode: SdkworkIotNode | null;
  selectedNodeId: string | null;
  visibleAlerts: SdkworkIotAlert[];
  visibleNodes: SdkworkIotNode[];
}

export interface SdkworkIotController {
  bootstrap(input?: GetSdkworkIotCatalogInput): Promise<SdkworkIotControllerState>;
  getState(): SdkworkIotControllerState;
  goToListPage(page: number): Promise<SdkworkIotControllerState>;
  refresh(input?: GetSdkworkIotCatalogInput): Promise<SdkworkIotControllerState>;
  selectNode(nodeId: string | null): void;
  service: SdkworkIotService;
  setAlertSeverity(severity: SdkworkIotAlertSeverity | "all"): void;
  setHealthLevel(healthLevel: SdkworkIotNodeHealthLevel | "all"): void;
  setKind(kind: SdkworkIotNodeKind | "all"): void;
  setSite(site: string | "all"): void;
  subscribe(listener: () => void): () => void;
}

export interface CreateSdkworkIotControllerOptions {
  initialState?: Partial<SdkworkIotControllerState>;
  service?: Partial<SdkworkIotService>;
}

function deriveVisibleNodes(
  catalog: SdkworkIotCatalogData,
  healthLevel: SdkworkIotNodeHealthLevel | "all",
  kind: SdkworkIotNodeKind | "all",
  site: string | "all",
): SdkworkIotNode[] {
  return catalog.nodes.filter((node) => {
    if (healthLevel !== "all" && node.healthLevel !== healthLevel) {
      return false;
    }

    if (kind !== "all" && node.kind !== kind) {
      return false;
    }

    if (site !== "all" && node.site !== site) {
      return false;
    }

    return true;
  });
}

function deriveVisibleAlerts(
  catalog: SdkworkIotCatalogData,
  visibleNodeIds: readonly string[],
  severity: SdkworkIotAlertSeverity | "all",
): SdkworkIotAlert[] {
  return catalog.alerts.filter((alert) => {
    if (!visibleNodeIds.includes(alert.nodeId)) {
      return false;
    }

    if (severity !== "all" && alert.severity !== severity) {
      return false;
    }

    return true;
  });
}

function resolveSelectedNodeId(
  nodes: readonly SdkworkIotNode[],
  selectedNodeId: string | null,
): string | null {
  if (selectedNodeId && nodes.some((node) => node.id === selectedNodeId)) {
    return selectedNodeId;
  }

  return nodes.find((node) => node.kind === "gateway")?.id ?? nodes[0]?.id ?? null;
}

function normalizeState(state: SdkworkIotControllerState): SdkworkIotControllerState {
  const visibleNodes = deriveVisibleNodes(
    state.catalog,
    state.activeHealthLevel,
    state.activeKind,
    state.activeSite,
  );
  const selectedNodeId = resolveSelectedNodeId(visibleNodes, state.selectedNodeId);
  const visibleAlerts = deriveVisibleAlerts(
    state.catalog,
    visibleNodes.map((node) => node.id),
    state.activeAlertSeverity,
  );

  return {
    ...state,
    selectedNode: visibleNodes.find((node) => node.id === selectedNodeId) ?? null,
    selectedNodeId,
    visibleAlerts,
    visibleNodes,
  };
}

export function createSdkworkIotController(
  options: CreateSdkworkIotControllerOptions = {},
): SdkworkIotController {
  const service: SdkworkIotService = options.service
    ? {
        ...createSdkworkIotService(),
        ...options.service,
      }
    : createSdkworkIotService();
  const fallbackCatalog = service.getEmptyCatalog();
  const listeners = new Set<() => void>();
  let state = normalizeState({
    activeAlertSeverity: "all",
    activeHealthLevel: "all",
    activeKind: "all",
    activeSite: "all",
    catalog: fallbackCatalog,
    isBootstrapped: false,
    isLoading: false,
    listPage: 1,
    selectedNode: null,
    selectedNodeId: fallbackCatalog.selectedNodeId,
    visibleAlerts: fallbackCatalog.alerts,
    visibleNodes: fallbackCatalog.nodes,
    ...options.initialState,
  });

  function emit(): void {
    listeners.forEach((listener) => listener());
  }

  function setState(
    next:
      | Partial<SdkworkIotControllerState>
      | ((currentState: SdkworkIotControllerState) => Partial<SdkworkIotControllerState>),
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
          selectedNodeId: catalog.selectedNodeId,
        });
        return state;
      } catch (error) {
        setState({
          isLoading: false,
          lastError: error instanceof Error ? error.message : "Failed to load IoT operations center.",
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
          nodeId: state.selectedNodeId,
          page: nextPage,
        });
        setState({
          catalog,
          isBootstrapped: true,
          isLoading: false,
          listPage: catalog.pageInfo?.page ?? nextPage,
          selectedNodeId: catalog.selectedNodeId,
        });
        return state;
      } catch (error) {
        setState({
          isLoading: false,
          lastError: error instanceof Error ? error.message : "Failed to load fleet page.",
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
        nodeId: state.selectedNodeId,
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

    selectNode(nodeId) {
      setState({
        selectedNodeId: nodeId,
      });
    },

    service,

    setAlertSeverity(severity) {
      setState({
        activeAlertSeverity: severity,
      });
    },

    setHealthLevel(healthLevel) {
      setState({
        activeHealthLevel: healthLevel,
      });
    },

    setKind(kind) {
      setState({
        activeKind: kind,
      });
    },

    setSite(site) {
      setState({
        activeSite: site,
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

export function useSdkworkIotController(
  controller?: SdkworkIotController,
  service?: Partial<SdkworkIotService>,
): SdkworkIotController {
  return useMemo(
    () => controller ?? createSdkworkIotController(service ? { service } : undefined),
    [controller, service],
  );
}

export function useSdkworkIotControllerState(
  controller: SdkworkIotController,
): SdkworkIotControllerState {
  return useSyncExternalStore(
    controller.subscribe,
    controller.getState,
    controller.getState,
  );
}
