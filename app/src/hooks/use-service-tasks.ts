import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { ServiceTaskItem } from "@/types/service-tasks";

export function useServiceTasks(serviceId: string) {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["service-tasks", serviceId],
        queryFn: () =>
            invoke<{ tasks: ServiceTaskItem[] }>("list_service_tasks", {
                url: agentUrl,
                token,
                serviceId,
            }),
        enabled: !!agentUrl && !!token && !!serviceId,
        refetchInterval: refreshInterval,
        select: (data) => data.tasks,
    });
}
