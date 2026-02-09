import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import { useSettings } from "@/hooks/use-settings";
import type { ServiceItem } from "@/types/services";

export function useServices(stack?: string) {
    const { agentUrl, token } = useAuth();
    const { refreshInterval } = useSettings();

    return useQuery({
        queryKey: ["services", stack ?? "all"],
        queryFn: () => invoke<{ services: ServiceItem[] }>("list_services", { url: agentUrl, token, stack: stack ?? null }),
        enabled: !!agentUrl && !!token,
        refetchInterval: refreshInterval,
        select: (data) => data.services,
    });
}
