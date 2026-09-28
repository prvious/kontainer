import { keepPreviousData, queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useAppSettings } from "@/hooks/use-app-settings";
import { useQueriesPaused } from "@/hooks/use-queries-paused";
import { inventoryErrorCode, type InventoryErrorCode } from "@/types/inventory";
import { controllerSettingsFromIpc, nodeListFromIpc, sampleSeriesFromIpc, type SampleWindow } from "@/types/nodes";
import { DEFAULT_APP_SETTINGS } from "@/types/settings";

type Report = ((profileId: string, code: InventoryErrorCode) => void) | undefined;

/** Invoke a Rust-authenticated read; failures become closed codes and custody errors are reported. */
async function authenticatedInvoke<T>(
    command: string,
    profileId: string,
    args: Record<string, unknown>,
    parse: (value: unknown) => T,
    report: Report,
): Promise<T> {
    try {
        return parse(await invoke<unknown>(command, { profileId, ...args }));
    } catch (error) {
        const code = inventoryErrorCode(error);
        report?.(profileId, code);
        throw code;
    }
}

/** `node` is a node id, or `"cluster"` for the cluster-wide series. */
export type SampleScope = string;

export const nodesQueries = {
    all: (profileId: string) => ["nodes", profileId] as const,
    list: (profileId: string, report: Report) =>
        queryOptions({
            queryKey: nodesQueries.all(profileId),
            queryFn: () => authenticatedInvoke("list_nodes", profileId, {}, nodeListFromIpc, report),
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
