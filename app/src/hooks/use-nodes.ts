import { keepPreviousData, queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { authenticatedInvoke, type ReportCustodyError as Report } from "@/hooks/authenticated-invoke";
import { clusterSummaryFromIpc, controllerSettingsFromIpc, nodeListFromIpc, sampleSeriesFromIpc, type SampleWindow } from "@/types/nodes";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

/** `node` is a node id, or `"cluster"` for the cluster-wide series. */
export type SampleScope = string;

export const nodesQueries = {
    all: (profileId: string) => ["nodes", profileId] as const,
    list: (profileId: string, report: Report) =>
        queryOptions({
            queryKey: nodesQueries.all(profileId),
            queryFn: () => authenticatedInvoke("list_nodes", profileId, {}, nodeListFromIpc, report),
        }),
    summary: (profileId: string, report: Report) =>
        queryOptions({
            queryKey: [...nodesQueries.all(profileId), "summary"] as const,
            queryFn: () => authenticatedInvoke("get_cluster_summary", profileId, {}, clusterSummaryFromIpc, report),
        }),
    samples: (profileId: string, scope: SampleScope, window: SampleWindow, bucket: string, report: Report) =>
        queryOptions({
            queryKey: [...nodesQueries.all(profileId), "samples", scope, window, bucket] as const,
            queryFn: () =>
                scope === "cluster"
                    ? authenticatedInvoke("get_cluster_samples", profileId, { window, bucket }, sampleSeriesFromIpc, report)
                    : authenticatedInvoke("get_node_samples", profileId, { nodeId: scope, window, bucket }, sampleSeriesFromIpc, report),
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
        placeholderData: keepPreviousData,
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
        placeholderData: keepPreviousData,
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
        placeholderData: keepPreviousData,
    });
}

export function useControllerSettings() {
    const { session, reportInventoryCustodyError } = useAuth();
    const paused = useQueriesPaused();
    const profileId = session?.profileId ?? "";
    return useQuery({
        queryKey: controllerSettingsKey(profileId),
        queryFn: () => authenticatedInvoke("get_settings", profileId, {}, controllerSettingsFromIpc, reportInventoryCustodyError),
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
            authenticatedInvoke("update_settings", profileId, { metricsRetentionDays }, controllerSettingsFromIpc, reportInventoryCustodyError),
        onSuccess: (settings) => queryClient.setQueryData(controllerSettingsKey(profileId), settings),
    });
}
