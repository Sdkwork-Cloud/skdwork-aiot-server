import { EmptyState } from "@sdkwork/ui-pc-react";
import type { SdkworkManagedDevice } from "../device";

export interface SdkworkDeviceInventoryGridProps {
  devices: readonly SdkworkManagedDevice[];
  onNavigate?: (route: string) => void;
  onSelectDevice?: (deviceId: string) => void;
  selectedDeviceId?: string | null;
}

export function SdkworkDeviceInventoryGrid({
  devices,
  onNavigate,
  onSelectDevice,
  selectedDeviceId,
}: SdkworkDeviceInventoryGridProps) {
  if (devices.length === 0) {
    return (
      <EmptyState
        description="No managed devices match the current device filters."
        title="No devices"
      />
    );
  }

  return (
    <div className="grid gap-4 xl:grid-cols-2">
      {devices.map((device) => {
        const isSelected = device.id === selectedDeviceId;

        return (
          <article
            className={`rounded-[1.45rem] border p-5 shadow-[var(--sdk-shadow-sm)] transition-all ${
              isSelected
                ? "border-cyan-400 bg-[linear-gradient(180deg,rgba(34,211,238,0.12),rgba(9,9,11,0.02))]"
                : "border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)]"
            }`}
            key={device.id}
          >
            <div className="flex items-start justify-between gap-4">
              <div>
                <div className="text-xs font-semibold uppercase tracking-[0.16em] text-[var(--sdk-color-text-muted)]">
                  {device.osName} | {device.hostname}
                </div>
                <h3 className="mt-2 text-xl font-semibold text-[var(--sdk-color-text-primary)]">{device.name}</h3>
              </div>
              {device.isPrimary ? (
                <span className="rounded-full bg-cyan-500/12 px-3 py-1 text-[0.68rem] font-semibold uppercase tracking-[0.16em] text-cyan-500">
                  Primary
                </span>
              ) : null}
            </div>

            <div className="mt-4 grid gap-3 sm:grid-cols-3 text-sm">
              <div>
                <div className="text-[var(--sdk-color-text-muted)]">Health</div>
                <div className="mt-1 font-medium capitalize text-[var(--sdk-color-text-primary)]">{device.healthLevel}</div>
              </div>
              <div>
                <div className="text-[var(--sdk-color-text-muted)]">Posture</div>
                <div className="mt-1 font-medium text-[var(--sdk-color-text-primary)]">
                  {device.postureScore === null ? '—' : `${device.postureScore}%`}
                </div>
              </div>
              <div>
                <div className="text-[var(--sdk-color-text-muted)]">Peripherals</div>
                <div className="mt-1 font-medium text-[var(--sdk-color-text-primary)]">{device.peripherals.length}</div>
              </div>
            </div>

            <div className="mt-4 flex flex-wrap gap-2">
              {device.labels.map((label) => (
                <span
                  className="rounded-full bg-[var(--sdk-color-surface-panel-muted)] px-2.5 py-1 text-xs text-[var(--sdk-color-text-secondary)]"
                  key={`${device.id}-${label}`}
                >
                  {label}
                </span>
              ))}
            </div>

            <div className="mt-5 flex flex-wrap gap-2">
              <button
                aria-label={`Open device route for ${device.name}`}
                className="rounded-[0.95rem] bg-[linear-gradient(135deg,#111827,#18181b_58%,#27272a)] px-4 py-2 text-xs font-semibold text-white"
                onClick={() => onNavigate?.(device.route)}
                type="button"
              >
                Open route
              </button>
              <button
                aria-label={`Select ${device.name}`}
                className="rounded-[0.95rem] border border-[var(--sdk-color-border-default)] px-4 py-2 text-xs font-semibold text-[var(--sdk-color-text-primary)]"
                onClick={() => onSelectDevice?.(device.id)}
                type="button"
              >
                Select {device.name}
              </button>
            </div>
          </article>
        );
      })}
    </div>
  );
}
