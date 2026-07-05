import { useEffect } from "react";
import { Cpu, RefreshCcw, ShieldCheck } from "lucide-react";
import {
  Button,
  LoadingBlock,
  StatusNotice,
} from "@sdkwork/ui-pc-react";
import type { SdkworkDeviceCapabilityArea } from "../device";
import type { SdkworkDeviceController } from "../device-controller";
import {
  useSdkworkDeviceController,
  useSdkworkDeviceControllerState,
} from "../device-controller";
import type { SdkworkDeviceService } from "../device-service";
import { SdkworkDeviceCapabilityRail } from "../components/DeviceCapabilityRail";
import { SdkworkDeviceHealthCards } from "../components/DeviceHealthCards";
import { SdkworkDeviceInventoryGrid } from "../components/DeviceInventoryGrid";

export interface SdkworkDevicePageProps {
  controller?: SdkworkDeviceController;
  onNavigate?: (route: string) => void;
  service?: Partial<SdkworkDeviceService>;
}

function labelArea(area: SdkworkDeviceCapabilityArea): string {
  return area.charAt(0).toUpperCase() + area.slice(1);
}

export function SdkworkDevicePage({
  controller: controllerProp,
  onNavigate,
  service,
}: SdkworkDevicePageProps) {
  const controller = useSdkworkDeviceController(controllerProp, service);
  const state = useSdkworkDeviceControllerState(controller);
  const capabilityAreas = Array.from(
    new Set(state.catalog.devices.flatMap((device) => device.capabilities.map((capability) => capability.area))),
  ) as SdkworkDeviceCapabilityArea[];

  useEffect(() => {
    if (!state.isBootstrapped && !state.isLoading) {
      void controller.bootstrap();
    }
  }, [controller, state.isBootstrapped, state.isLoading]);

  return (
    <div className="h-full overflow-y-auto px-4 py-4 sm:px-5 sm:py-5">
      <div className="mx-auto max-w-[96rem] space-y-5">
        <section className="grid gap-5 xl:grid-cols-[minmax(0,1.55fr)_minmax(22rem,0.85fr)]">
          <div className="overflow-hidden rounded-[2rem] bg-[radial-gradient(circle_at_top_right,rgba(34,211,238,0.16),transparent_28%),linear-gradient(135deg,#09090b,#111827_48%,#27272a)] px-6 py-7 text-white shadow-[var(--sdk-shadow-lg)]">
            <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
              <div className="max-w-3xl">
                <div className="inline-flex items-center gap-2 rounded-full bg-white/10 px-3 py-1 text-[0.7rem] font-semibold uppercase tracking-[0.18em] text-white/72">
                  <Cpu className="h-3.5 w-3.5" />
                  Managed Hardware
                </div>
                <h1 className="mt-4 text-4xl font-semibold tracking-tight">Device Center</h1>
                <p className="mt-3 text-sm leading-7 text-white/72">
                  Monitor machine posture, capability readiness, and peripheral health from a reusable device operations surface.
                </p>
              </div>

              <div className="flex flex-wrap gap-3">
                <Button onClick={() => void controller.refresh()} type="button" variant="outline">
                  <RefreshCcw className="mr-2 h-4 w-4" />
                  Refresh devices
                </Button>
              </div>
            </div>

            <div className="mt-8 grid gap-4 md:grid-cols-3">
              <div className="rounded-[1.4rem] bg-white/8 p-5">
                <div className="text-sm text-white/65">Managed devices</div>
                <div className="mt-3 text-3xl font-semibold tracking-tight">{state.catalog.summary.totalDevices}</div>
              </div>
              <div className="rounded-[1.4rem] bg-white/8 p-5">
                <div className="text-sm text-white/65">Connected peripherals</div>
                <div className="mt-3 text-3xl font-semibold tracking-tight">{state.catalog.summary.connectedPeripherals}</div>
              </div>
              <div className="rounded-[1.4rem] bg-white/8 p-5">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="text-sm text-white/65">Posture average</div>
                    <div className="mt-3 text-3xl font-semibold tracking-tight">{state.catalog.summary.postureAverage}%</div>
                  </div>
                  <ShieldCheck className="h-5 w-5 text-white/80" />
                </div>
              </div>
            </div>
          </div>

          <SdkworkDeviceCapabilityRail device={state.selectedDevice} />
        </section>

        {state.isLoading && !state.isBootstrapped ? <LoadingBlock label="Loading device center..." /> : null}

        {state.lastError ? (
          <StatusNotice title="Device center error" tone="danger">
            {state.lastError}
          </StatusNotice>
        ) : null}

        <SdkworkDeviceHealthCards summary={state.catalog.summary} />

        <section className="rounded-[1.65rem] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)] p-5 shadow-[var(--sdk-shadow-sm)]">
          <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div className="flex flex-wrap gap-2">
              {[
                { id: "all", label: "All" },
                { id: "healthy", label: "Healthy" },
                { id: "warning", label: "Warning" },
                { id: "critical", label: "Critical" },
              ].map((option) => (
                <button
                  className={`rounded-full px-3 py-1.5 text-xs font-semibold uppercase tracking-[0.14em] transition-colors ${
                    state.activeHealthLevel === option.id
                      ? "bg-cyan-500 text-white"
                      : "bg-[var(--sdk-color-surface-panel-muted)] text-[var(--sdk-color-text-secondary)]"
                  }`}
                  key={option.id}
                  onClick={() => controller.setHealthLevel(option.id as typeof state.activeHealthLevel)}
                  type="button"
                >
                  {option.label}
                </button>
              ))}
            </div>

            <div className="flex flex-wrap gap-2">
              <button
                className={`rounded-[0.85rem] border px-3 py-1.5 text-xs font-semibold transition-colors ${
                  state.activeArea === "all"
                    ? "border-sky-500 bg-sky-500/10 text-sky-500"
                    : "border-[var(--sdk-color-border-default)] text-[var(--sdk-color-text-secondary)]"
                }`}
                onClick={() => controller.setArea("all")}
                type="button"
              >
                All capabilities
              </button>
              {capabilityAreas.map((area) => (
                <button
                  className={`rounded-[0.85rem] border px-3 py-1.5 text-xs font-semibold transition-colors ${
                    state.activeArea === area
                      ? "border-sky-500 bg-sky-500/10 text-sky-500"
                      : "border-[var(--sdk-color-border-default)] text-[var(--sdk-color-text-secondary)]"
                  }`}
                  key={area}
                  onClick={() => controller.setArea(area)}
                  type="button"
                >
                  {labelArea(area)}
                </button>
              ))}
            </div>
          </div>

          <div className="mt-5">
            <SdkworkDeviceInventoryGrid
              devices={state.visibleDevices}
              onNavigate={onNavigate}
              onSelectDevice={(deviceId) => controller.selectDevice(deviceId)}
              selectedDeviceId={state.selectedDeviceId}
            />
            {state.catalog.pageInfo ? (
              <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-[var(--sdk-color-border-default)] pt-4">
                <div className="text-xs text-[var(--sdk-color-text-secondary)]">
                  Page {state.catalog.pageInfo.page}
                  {typeof state.catalog.pageInfo.total === "number"
                    ? ` of ${Math.max(1, Math.ceil(state.catalog.pageInfo.total / state.catalog.pageInfo.pageSize))}`
                    : ""}
                  {" · "}
                  {state.catalog.devices.length} device(s) on this page
                </div>
                <div className="flex gap-2">
                  <Button
                    disabled={state.isLoading || state.listPage <= 1}
                    onClick={() => void controller.goToListPage(state.listPage - 1)}
                    type="button"
                    variant="outline"
                  >
                    Previous
                  </Button>
                  <Button
                    disabled={state.isLoading || !state.catalog.pageInfo.hasMore}
                    onClick={() => void controller.goToListPage(state.listPage + 1)}
                    type="button"
                    variant="outline"
                  >
                    Next
                  </Button>
                </div>
              </div>
            ) : null}
          </div>
        </section>
      </div>
    </div>
  );
}
