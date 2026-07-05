import type { SdkworkIotNode } from "../iot";

export interface SdkworkIotFleetGridProps {
  nodes: readonly SdkworkIotNode[];
  onNavigate?: (route: string) => void;
  onSelectNode?: (nodeId: string) => void;
  selectedNodeId?: string | null;
}

function labelHealth(value: SdkworkIotNode["healthLevel"]): string {
  if (value === "critical") {
    return "Critical";
  }

  if (value === "warning") {
    return "Warning";
  }

  return "Healthy";
}

export function SdkworkIotFleetGrid({
  nodes,
  onNavigate,
  onSelectNode,
  selectedNodeId,
}: SdkworkIotFleetGridProps) {
  if (nodes.length === 0) {
    return (
      <div className="rounded-[1rem] border border-dashed border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel-muted)] px-4 py-8 text-center text-sm text-[var(--sdk-color-text-secondary)]">
        No IoT nodes match the current fleet filters.
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {nodes.map((node) => {
        const isSelected = selectedNodeId === node.id;
        const healthClasses = node.healthLevel === "critical"
          ? "bg-rose-500/12 text-rose-500"
          : node.healthLevel === "warning"
            ? "bg-amber-500/12 text-amber-500"
            : "bg-emerald-500/12 text-emerald-500";

        return (
          <article
            className={`rounded-[1.2rem] border p-4 transition-colors ${
              isSelected
                ? "border-cyan-500/60 bg-cyan-500/8"
                : "border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)]"
            }`}
            key={node.id}
          >
            <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <h3 className="text-base font-semibold text-[var(--sdk-color-text-primary)]">{node.name}</h3>
                  <span className={`rounded-full px-2 py-0.5 text-xs font-semibold ${healthClasses}`}>
                    {labelHealth(node.healthLevel)}
                  </span>
                  <span className="rounded-full bg-[var(--sdk-color-surface-panel-muted)] px-2 py-0.5 text-xs font-semibold capitalize text-[var(--sdk-color-text-secondary)]">
                    {node.kind}
                  </span>
                </div>
                <p className="mt-1 text-sm text-[var(--sdk-color-text-secondary)]">
                  {node.site} | Firmware {node.firmwareVersion} | Posture {node.postureScore === null ? '—' : `${node.postureScore}%`}
                </p>
              </div>

              <div className="flex flex-wrap gap-2">
                <button
                  className="rounded-[0.8rem] border border-[var(--sdk-color-border-default)] px-3 py-1.5 text-xs font-semibold text-[var(--sdk-color-text-secondary)] transition-colors hover:border-cyan-500/50 hover:text-cyan-500"
                  onClick={() => onNavigate?.(node.route)}
                  type="button"
                >
                  Open node route for {node.name}
                </button>
                <button
                  className={`rounded-[0.8rem] px-3 py-1.5 text-xs font-semibold transition-colors ${
                    isSelected
                      ? "bg-cyan-500 text-white"
                      : "bg-[var(--sdk-color-surface-panel-muted)] text-[var(--sdk-color-text-secondary)] hover:text-[var(--sdk-color-text-primary)]"
                  }`}
                  onClick={() => onSelectNode?.(node.id)}
                  type="button"
                >
                  Select {node.name}
                </button>
              </div>
            </div>
          </article>
        );
      })}
    </div>
  );
}
