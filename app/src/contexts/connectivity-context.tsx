import { useMemo, useState, type ReactNode } from "react";
import { ConnectivityContext, type ConnectivityContextValue } from "@/contexts/connectivity-context-value";
import { useControllerHealth } from "@/hooks/session/use-controller-health";
import type { ProbeView } from "@/types/session";

const UNREACHABLE: ProbeView = { status: "unreachable" };

/** The thrown probe, or a bare unreachable one when the invoke itself failed; stable across renders. */
function failedProbe(reason: unknown): ProbeView {
    if (typeof reason === "object" && reason !== null && (reason as ProbeView).status === "unreachable") return reason as ProbeView;
    return UNREACHABLE;
}

/**
 * Controller reachability for the authenticated layout (spec 14), derived from the health probe:
 * unreachable once a probe fetch has failed all three attempts, reachable again when one answers.
 * `reconnectedAt` is recorded while rendering the flip back, not in an effect.
 */
export function ConnectivityProvider({ enabled, children }: { enabled: boolean; children: ReactNode }) {
    const health = useControllerHealth(enabled);
    const reachable = !health.isError;
    const [wasReachable, setWasReachable] = useState(reachable);
    const [reconnectedAt, setReconnectedAt] = useState<number | null>(null);
    if (wasReachable !== reachable) {
        setWasReachable(reachable);
        if (reachable) setReconnectedAt(health.dataUpdatedAt);
    }

    const failing = health.isError || health.failureCount > 0;
    const probe = failing ? failedProbe(health.failureReason ?? health.error) : health.data;
    const { refetch, isFetching } = health;
    const value = useMemo<ConnectivityContextValue>(
        () => ({
            probe,
            reachable,
            reconnectedAt,
            retry: () => void refetch(),
            retrying: isFetching,
        }),
        [probe, reachable, reconnectedAt, refetch, isFetching],
    );
    return <ConnectivityContext value={value}>{children}</ConnectivityContext>;
}
