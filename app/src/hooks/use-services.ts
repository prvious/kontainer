import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { ServiceItem } from "@/types/services";

export function useServices() {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["services"],
        queryFn: () => invoke<{ services: ServiceItem[] }>("list_services", { url: agentUrl, token }),
        enabled: !!agentUrl && !!token,
        refetchInterval: 5000,
        select: (data) => data.services,
    });
}
