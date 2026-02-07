import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { TaskDetail } from "@/types/task-detail";

export function useTaskDetail(taskId: string | null) {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["task-detail", taskId],
        queryFn: () =>
            invoke<TaskDetail>("inspect_task", {
                url: agentUrl,
                token,
                taskId,
            }),
        enabled: !!agentUrl && !!token && !!taskId,
    });
}
