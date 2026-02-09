import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { NodeItem } from "@/types/nodes";

export function useNodes() {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["nodes"],
        queryFn: () => invoke<{ nodes: NodeItem[] }>("list_nodes", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: refreshInterval,
        select: (data) => data.nodes,
    });
}
