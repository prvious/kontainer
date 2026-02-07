import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { NodeItem } from "@/types/nodes";

export function useNodes() {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["nodes"],
        queryFn: () => invoke<{ nodes: NodeItem[] }>("list_nodes", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: 5000,
        select: (data) => data.nodes,
    });
}
