import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { NodeTaskItem } from "@/types/node-tasks";

export function useNodeTasks(nodeId: string | null, options?: { refetchInterval?: number | false }) {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["node-tasks", nodeId],
        queryFn: () =>
            invoke<NodeTaskItem[]>("list_node_tasks", {
                url: agentUrl,
                token,
                nodeId,
            }),
        enabled: !!agentUrl && !!token && !!nodeId,
        refetchInterval: options?.refetchInterval,
    });
}
