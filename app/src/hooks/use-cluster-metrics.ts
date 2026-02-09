import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { ClusterMetricsResponse } from "@/types/metrics";

export function useClusterMetrics() {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["cluster-metrics"],
        queryFn: () =>
            invoke<ClusterMetricsResponse>("list_cluster_metrics", {
                url: agentUrl,
                token,
            }),
        enabled: !!agentUrl && !!token,
        refetchInterval: refreshInterval,
    });
}
