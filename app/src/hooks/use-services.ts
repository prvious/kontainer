import { skipToken, useQuery } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke, type ReportCustodyError } from "@/hooks/authenticated-invoke";
import { useStartOperation, type WithIdempotencyKey } from "@/hooks/use-operations";
import type { ServiceDetailView, ServiceListView } from "@/types/services";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

export const servicesKey = (profileId: string) => ["services", profileId] as const;

/** The service list query's function, for callers that fetch it imperatively too. */
export const listServices = (profileId: string, report: ReportCustodyError) =>
    authenticatedInvoke<ServiceListView>("list_services", profileId, { stack: null }, report);

/** While the swarm has no quorum the list asks again every ten seconds. */
export const QUORUM_RETRY_MS = 10_000;

/** Every Swarm service, polled at the refresh interval (every 10 s while it errors). */
export function useServices() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const settings = useAppSettings();
    const profileId = session?.profileId ?? "";
    const interval = settings.data?.refreshInterval ?? DEFAULT_APP_SETTINGS.refreshInterval ?? 5000;
    return useQuery({
        queryKey: servicesKey(profileId),
        queryFn: () => listServices(profileId, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        refetchInterval: (query) => (query.state.status === "error" ? QUORUM_RETRY_MS : interval),
        retry: false,
    });
}

/** One service with its tasks, fetched only while `serviceId` is set (a hovered replica count). */
export function useService(serviceId: string | null) {
    const { session, reportInventoryCustodyError } = useAuth();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: [...servicesKey(profileId), "detail", serviceId] as const,
        queryFn:
            serviceId && session
                ? () => authenticatedInvoke<ServiceDetailView>("get_service", profileId, { serviceId }, reportInventoryCustodyError)
                : skipToken,
        retry: false,
        staleTime: 5_000,
    });
}

interface ServiceTarget extends WithIdempotencyKey {
    serviceId: string;
    name: string;
    version: number;
}

export interface ScaleServiceVariables extends ServiceTarget {
    replicas: number;
    /** For the title only. */
    from: number;
}

/** Scale one replicated service; the operation reports convergence. */
export function useScaleService() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<ScaleServiceVariables>({
        command: "scale_service",
        scope: "services",
        title: ({ name, from, replicas }) => `Scaling ${name} · ${from} → ${replicas} replicas`,
        success: ({ name, replicas }) => ({ title: `Scaled ${name}`, description: `${replicas} of ${replicas} tasks running.` }),
        targets: ({ serviceId }) => [`scale:${serviceId}`],
        invalidate: [servicesKey(profileId)],
        args: ({ serviceId, replicas, version, idempotencyKey }) => ({ serviceId, replicas, version, idempotencyKey }),
    });
}

/** Restart every task (force update); waits for fresh tasks. */
export function useRestartService() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<ServiceTarget>({
        command: "restart_service",
        scope: "services",
        title: ({ name }) => `Restarting ${name}`,
        success: ({ name }) => ({ title: `Restarted ${name}`, description: "Every task was replaced." }),
        targets: ({ serviceId }) => [`update:${serviceId}`],
        invalidate: [servicesKey(profileId)],
        args: ({ serviceId, version, idempotencyKey }) => ({ serviceId, version, idempotencyKey }),
    });
}

/** Roll back to the previous spec. */
export function useRollbackService() {
    const { session } = useAuth();
    const profileId = session?.profileId ?? "";
    return useStartOperation<ServiceTarget>({
        command: "rollback_service",
        scope: "services",
        title: ({ name }) => `Rolling back ${name}`,
        success: ({ name }) => ({ title: `Rolled back ${name}`, description: "It runs its previous spec." }),
        targets: ({ serviceId }) => [`update:${serviceId}`],
        invalidate: [servicesKey(profileId)],
        args: ({ serviceId, version, idempotencyKey }) => ({ serviceId, version, idempotencyKey }),
    });
}
