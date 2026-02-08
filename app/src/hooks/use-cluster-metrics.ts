import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { ClusterMetricsResponse } from "@/types/metrics";

export function useClusterMetrics() {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["cluster-metrics"],
        queryFn: () =>
            invoke<ClusterMetricsResponse>("list_cluster_metrics", {
                url: agentUrl,
                token,
            }),
        enabled: !!agentUrl && !!token,
        refetchInterval: 5000,
    });
}
