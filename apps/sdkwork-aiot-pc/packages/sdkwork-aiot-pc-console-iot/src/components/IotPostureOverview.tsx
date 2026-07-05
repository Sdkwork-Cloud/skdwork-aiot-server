import type {
  SdkworkIotRemoteControlIntent,
  SdkworkIotSitePosture,
} from "../iot";

export interface SdkworkIotPostureOverviewProps {
  intents: readonly SdkworkIotRemoteControlIntent[];
  onTriggerIntent?: (intentId: SdkworkIotRemoteControlIntent["id"]) => void;
  sitePosture: readonly SdkworkIotSitePosture[];
}

function labelPostureLevel(value: SdkworkIotSitePosture["level"]): string {
  if (value === "secure") {
    return "Secure";
  }

  if (value === "degraded") {
    return "Degraded";
  }

  return "Vulnerable";
}

export function SdkworkIotPostureOverview({
  intents,
  onTriggerIntent,
  sitePosture,
}: SdkworkIotPostureOverviewProps) {
  return (
    <section className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
      <div className="space-y-3 rounded-[1.2rem] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)] p-4">
        <h3 className="text-sm font-semibold uppercase tracking-[0.16em] text-[var(--sdk-color-text-muted)]">
          Site posture
        </h3>
        <div className="grid gap-2 md:grid-cols-2">
          {sitePosture.map((site) => {
            const tone = site.level === "secure"
              ? "bg-emerald-500/12 text-emerald-500"
              : site.level === "degraded"
                ? "bg-amber-500/12 text-amber-500"
                : "bg-rose-500/12 text-rose-500";

            return (
              <article
                className="rounded-[0.95rem] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel-muted)] p-3"
                key={site.site}
              >
                <div className="flex items-center justify-between gap-3">
                  <h4 className="text-sm font-semibold text-[var(--sdk-color-text-primary)]">{site.site}</h4>
                  <span className={`rounded-full px-2 py-0.5 text-[0.7rem] font-semibold uppercase tracking-[0.08em] ${tone}`}>
                    {labelPostureLevel(site.level)}
                  </span>
                </div>
                <p className="mt-2 text-xs text-[var(--sdk-color-text-secondary)]">
                  Posture {site.postureScore}% | Nodes {site.nodeCount}
                </p>
              </article>
            );
          })}
        </div>
      </div>

      <div className="space-y-3 rounded-[1.2rem] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel)] p-4">
        <h3 className="text-sm font-semibold uppercase tracking-[0.16em] text-[var(--sdk-color-text-muted)]">
          Remote control intents
        </h3>
        {intents.map((intent) => (
          <article
            className="rounded-[0.95rem] border border-[var(--sdk-color-border-default)] bg-[var(--sdk-color-surface-panel-muted)] p-3"
            key={intent.id}
          >
            <h4 className="text-sm font-semibold text-[var(--sdk-color-text-primary)]">{intent.label}</h4>
            <p className="mt-1 text-xs leading-5 text-[var(--sdk-color-text-secondary)]">{intent.description}</p>
            <button
              className="mt-2 rounded-[0.7rem] border border-[var(--sdk-color-border-default)] px-2.5 py-1 text-xs font-semibold text-[var(--sdk-color-text-secondary)] transition-colors hover:border-cyan-500/50 hover:text-cyan-500"
              onClick={() => onTriggerIntent?.(intent.id)}
              type="button"
            >
              Go to {intent.label}
            </button>
          </article>
        ))}
      </div>
    </section>
  );
}
