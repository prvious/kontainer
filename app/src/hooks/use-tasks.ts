import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { TaskItem } from "@/types/tasks";

export function useTasks() {
	const { agentUrl, token } = useAuth();
	const { refreshInterval } = useSettings();

	return useQuery({
		queryKey: ["tasks"],
		queryFn: () => invoke<{ tasks: TaskItem[] }>("list_tasks", { url: agentUrl, token }),
		enabled: !!agentUrl && !!token,
		refetchInterval: refreshInterval,
		select: (data) => data.tasks,
	});
}
