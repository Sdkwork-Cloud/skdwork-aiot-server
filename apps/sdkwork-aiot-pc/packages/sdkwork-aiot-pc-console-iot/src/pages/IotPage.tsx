import { useEffect } from "react";
import { Activity, RadioTower, RefreshCcw } from "lucide-react";
import {
  Button,
  LoadingBlock,
  StatusNotice,
} from "@sdkwork/ui-pc-react";
import type { SdkworkIotAlertSeverity, SdkworkIotNodeHealthLevel, SdkworkIotNodeKind } from "../iot";
import type { SdkworkIotController } from "../iot-controller";
import {
  useSdkworkIotController,
  useSdkworkIotControllerState,
} from "../iot-controller";
import type { SdkworkIotService } from "../iot-service";
import { SdkworkIotAlertTimeline } from "../components/IotAlertTimeline";
import { SdkworkIotFleetGrid } from "../components/IotFleetGrid";
import { SdkworkIotPostureOverview } from "../components/IotPostureOverview";
import { SdkworkFirmwareArtifactUploadPanel } from "../components/FirmwareArtifactUploadPanel";

export interface SdkworkIotPageProps {
  controller?: SdkworkIotController;
  onNavigate?: (route: string) => void;
  service?: Partial<SdkworkIotService>;
}

const HEALTH_FILTERS: Array<{ id: SdkworkIotNodeHealthLevel | "all"; label: string }> = [
  { id: "all", label: "All" },
  { id: "healthy", label: "Healthy" },
  { id: "warning", label: "Warning" },
  { id: "critical", label: "Critical" },
];

const KIND_FILTERS: Array<{ id: SdkworkIotNodeKind | "all"; label: string }> = [
  { id: "all", label: "All nodes" },
  { id: "gateway", label: "Gateway" },
  { id: "sensor", label: "Sensor" },
];

const ALERT_FILTERS: Array<{ id: SdkworkIotAlertSeverity | "all"; label: string }> = [
  { id: "all", label: "All alerts" },
  { id: "critical", label: "Critical alerts" },
  { id: "warning", label: "Warning alerts" },
  { id: "info", label: "Info alerts" },
];

export function SdkworkIotPage({
  controller: controllerProp,
  onNavigate,
  service,
}: SdkworkIotPageProps) {
  const controller = useSdkworkIotController(controllerProp, service);
  const state = useSdkworkIotControllerState(controller);
  const siteFilters = Array.from(new Set(state.catalog.nodes.map((node) => node.site)));

  useEffect(() => {
    if (!state.isBootstrapped && !state.isLoading) {
      void controller.bootstrap();
    }
  }, [controller, state.isBootstrapped, state.isLoading]);

  return (
    <div className="h-full overflow-y-auto px-4 py-4 sm:px-5 sm:py-5">
      <div className="mx-auto max-w-[96rem] space-y-5">
        <section className="grid gap-5 xl:grid-cols-[minmax(0,1.5fr)_minmax(22rem,0.9fr)]">
          <div className="overflow-hidden rounded-[2rem] bg-[radial-gradient(circle_at_top_right,rgba(34,211,238,0.18),transparent_28%),linear-gradient(135deg,#09090b,#0f172a_46%,#27272a)] px-6 py-7 text-white shadow-[var(--sdk-shadow-lg)]">
            <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
              <div className="max-w-3xl">
                <div className="inline-flex items-center gap-2 rounded-full bg-white/10 px-3 py-1 text-[0.7rem] font-semibold uppercase tracking-[0.18em] text-white/72">
                  <RadioTower className="h-3.5 w-3.5" />
                  Edge Fleet
                </div>
                <h1 className="mt-4 text-4xl font-semibold tracking-tight">IoT Operations Center</h1>
                <p className="mt-3 text-sm leading-7 text-white/72">
                  Operate gateways, sensors, and alerts from a reusable fleet surface tuned for high-capability control-room workflows.
                </p>
              </div>

              <div className="flex flex-wrap gap-3">
                <Button onClick={() => void controller.refresh()} type="button" variant="outline">
                  <RefreshCcw className="mr-2 h-4 w-4" />
                  Refresh fleet
                </Button>
              </div>
            </div>

            <div className="mt-8 grid gap-4 md:grid-cols-4">
              <div className="rounded-[1.4rem] bg-white/8 p-5">
                <div className="text-sm text-white/65">Total nodes</div>
                <div className="mt-3 text-3xl font-semibold tracking-tight">{state.catalog.summary.totalNodes}</div>
              </div>
              <div className="rounded-[1.4rem] bg-white/8 p-5">
                <div className="text-sm text-white/65">Online nodes</div>
                <div className="mt-3 text-3xl font-semibold tracking-tight">{state.catalog.summary.onlineNodes}</div>
              </div>
              <div className="rounded-[1.4rem] bg-white/8 p-5">
                <div className="text-sm text-white/65">Unacknowledged alerts</div>
                <div className="mt-3 text-3xl font-semibold tracking-tight">{state.catalog.summary.unacknowledgedAlerts}</div>
              </div>
              <div className="rounded-[1.4rem] bg-white/8 p-5">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="text-sm text-white/65">Fleet posture</div>
                    <div className="mt-3 text-3xl font-semibold tracking-tight">{state.catalog.summary.postureAverage}%</div>
                  </div>
                  <Activity className="h-5 w-5 text-white/80" />
                </div>
              </div>
            </div>
          </div>

          <SdkworkIotPostureOverview
            intents={state.catalog.remoteControlIntents}
            onTriggerIntent={(intentId) => {
              const intent = state.catalog.remoteControlIntents.find((item) => item.id === intentId);
              if (intent) {
                onNavigate?.(intent.routeIntent.route);
              }
            }}
            sitePosture={state.catalog.sitePosture}
          />
        </section>

        {state.isLoading && !state.isBootstrapped ? <LoadingBlock label="Loading IoT operations center..." /> : null}

        {state.lastError ? (
          <StatusNotice title="IoT operations error" tone="danger">
            {state.lastError}
          </StatusNotice>
        ) : null}

        <section className="rounded-[1.65rem] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)] p-5 shadow-[var(--sdk-shadow-sm)]">
          <div className="flex flex-col gap-4">
            <div className="flex flex-wrap gap-2">
              {HEALTH_FILTERS.map((option) => (
                <button
                  className={`rounded-full px-3 py-1.5 text-xs font-semibold uppercase tracking-[0.14em] transition-colors ${
                    state.activeHealthLevel === option.id
                      ? "bg-cyan-500 text-white"
                      : "bg-[var(--sdk-color-surface-panel-muted)] text-[var(--sdk-color-text-secondary)]"
                  }`}
                  key={option.id}
                  onClick={() => controller.setHealthLevel(option.id)}
                  type="button"
                >
                  {option.label}
                </button>
              ))}
            </div>

            <div className="flex flex-wrap gap-2">
              {KIND_FILTERS.map((option) => (
                <button
                  className={`rounded-[0.85rem] border px-3 py-1.5 text-xs font-semibold transition-colors ${
                    state.activeKind === option.id
                      ? "border-sky-500 bg-sky-500/10 text-sky-500"
                      : "border-[var(--sdk-color-border-default)] text-[var(--sdk-color-text-secondary)]"
                  }`}
                  key={option.id}
                  onClick={() => controller.setKind(option.id)}
                  type="button"
                >
                  {option.label}
                </button>
              ))}
              <button
                className={`rounded-[0.85rem] border px-3 py-1.5 text-xs font-semibold transition-colors ${
                  state.activeSite === "all"
                    ? "border-sky-500 bg-sky-500/10 text-sky-500"
                    : "border-[var(--sdk-color-border-default)] text-[var(--sdk-color-text-secondary)]"
                }`}
                onClick={() => controller.setSite("all")}
                type="button"
              >
                All sites
              </button>
              {siteFilters.map((site) => (
                <button
                  className={`rounded-[0.85rem] border px-3 py-1.5 text-xs font-semibold transition-colors ${
                    state.activeSite === site
                      ? "border-sky-500 bg-sky-500/10 text-sky-500"
                      : "border-[var(--sdk-color-border-default)] text-[var(--sdk-color-text-secondary)]"
                  }`}
                  key={site}
                  onClick={() => controller.setSite(site)}
                  type="button"
                >
                  {site}
                </button>
              ))}
            </div>
          </div>

          <div className="mt-5">
            <SdkworkIotFleetGrid
              nodes={state.visibleNodes}
              onNavigate={onNavigate}
              onSelectNode={(nodeId) => controller.selectNode(nodeId)}
              selectedNodeId={state.selectedNodeId}
            />
          </div>
        </section>

        <SdkworkFirmwareArtifactUploadPanel />

        <section className="rounded-[1.65rem] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)] p-5 shadow-[var(--sdk-shadow-sm)]">
          <div className="flex flex-wrap gap-2">
            {ALERT_FILTERS.map((option) => (
              <button
                className={`rounded-[0.85rem] border px-3 py-1.5 text-xs font-semibold transition-colors ${
                  state.activeAlertSeverity === option.id
                    ? "border-rose-500 bg-rose-500/10 text-rose-500"
                    : "border-[var(--sdk-color-border-default)] text-[var(--sdk-color-text-secondary)]"
                }`}
                key={option.id}
                onClick={() => controller.setAlertSeverity(option.id)}
                type="button"
              >
                {option.label}
              </button>
            ))}
          </div>

          <div className="mt-5">
            <SdkworkIotAlertTimeline alerts={state.visibleAlerts} onNavigate={onNavigate} />
          </div>
        </section>
      </div>
    </div>
  );
}
