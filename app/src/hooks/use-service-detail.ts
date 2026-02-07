import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { ServiceDetail } from "@/types/service-detail";

export function useServiceDetail(serviceId: string) {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["service", serviceId],
        queryFn: () =>
            invoke<ServiceDetail>("inspect_service", {
                url: agentUrl,
                token,
                serviceId,
            }),
        enabled: !!agentUrl && !!token && !!serviceId,
    });
}
