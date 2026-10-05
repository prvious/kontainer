import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke, type ReportCustodyError as Report } from "@/hooks/authenticated-invoke";
import type { ClusterSummary, ControllerSettings, ControllerStatusView, NodeList, SampleSeries, SampleWindow } from "@/types/nodes";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

/** The controller serves only the cluster-wide series (ADR 0039). */
type SampleScope = "cluster";

export const nodesQueries = {
    all: (profileId: string) => ["nodes", profileId] as const,
    list: (profileId: string, report: Report) =>
        queryOptions({
            queryKey: nodesQueries.all(profileId),
            queryFn: () => authenticatedInvoke<NodeList>("list_nodes", profileId, {}, report),
        }),
    summary: (profileId: string, report: Report) =>
        queryOptions({
            queryKey: [...nodesQueries.all(profileId), "summary"] as const,
            queryFn: () => authenticatedInvoke<ClusterSummary>("get_cluster_summary", profileId, {}, report),
        }),
    samples: (profileId: string, scope: SampleScope, window: SampleWindow, bucket: string, report: Report) =>
        queryOptions({
            queryKey: [...nodesQueries.all(profileId), "samples", scope, window, bucket] as const,
            queryFn: () => authenticatedInvoke<SampleSeries>("get_cluster_samples", profileId, { window, bucket }, report),
        }),
};

export const controllerSettingsKey = (profileId: string) => ["controller-settings", profileId] as const;

/** Polling cadence from the desktop refresh setting. */
function useRefreshInterval(): number {
    const settings = useAppSettings();
    return settings.data?.refreshInterval ?? DEFAULT_APP_SETTINGS.refreshInterval ?? 5000;
}

/** Membership for the signed-in profile, polled at the refresh interval. */
export function useNodes() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const interval = useRefreshInterval();
    const profileId = session?.profileId ?? "";
    return useQuery({
        ...nodesQueries.list(profileId, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        refetchInterval: interval,
        retry: false,
    });
}

/** Node counts, quorum, and load for the strip, the Overview, and the status bar. */
export function useClusterSummary() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const interval = useRefreshInterval();
    const profileId = session?.profileId ?? "";
    return useQuery({
        ...nodesQueries.summary(profileId, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        refetchInterval: interval,
        retry: false,
    });
}

export function useSamples(scope: SampleScope, window: SampleWindow, bucket: string) {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const interval = useRefreshInterval();
    const profileId = session?.profileId ?? "";
    return useQuery({
        ...nodesQueries.samples(profileId, scope, window, bucket, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        refetchInterval: interval,
        retry: false,
    });
}

const controllerStatusKey = (profileId: string) => ["controller-status", profileId] as const;

/** The controller's version, log filter, and start time; they change only when it restarts. */
export function useControllerStatus() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: controllerStatusKey(profileId),
        queryFn: () => authenticatedInvoke<ControllerStatusView>("get_status", profileId, {}, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        staleTime: 60_000,
        refetchInterval: 60_000,
        retry: false,
    });
}

export function useControllerSettings() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: controllerSettingsKey(profileId),
        queryFn: () => authenticatedInvoke<ControllerSettings>("get_settings", profileId, {}, reportInventoryCustodyError),
        enabled: Boolean(session) && !paused,
        retry: false,
    });
}

export function useUpdateControllerSettings() {
    const { session, reportInventoryCustodyError } = useAuth();
    const queryClient = useQueryClient();
    const profileId = session?.profileId ?? "";
    return useMutation({
        mutationFn: (metricsRetentionDays: number) =>
            authenticatedInvoke<ControllerSettings>("update_settings", profileId, { metricsRetentionDays }, reportInventoryCustodyError),
        onSuccess: (settings) => queryClient.setQueryData(controllerSettingsKey(profileId), settings),
    });
}
