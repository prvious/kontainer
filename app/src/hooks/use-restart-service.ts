import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface RestartServiceResponse {
    status: string;
}

export function useRestartService(serviceId: string) {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: () =>
            invoke<RestartServiceResponse>("restart_service", {
                url: agentUrl,
                token,
                serviceId,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["service", serviceId] });
            queryClient.invalidateQueries({ queryKey: ["services"] });
        },
    });
}
