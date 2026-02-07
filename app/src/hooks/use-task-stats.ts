import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { TaskStats } from "@/types/task-stats";

export function useTaskStats() {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["taskStats"],
        queryFn: () => invoke<{ taskStats: TaskStats }>("list_task_stats", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: 5000,
        select: (data) => data.taskStats,
    });
}
