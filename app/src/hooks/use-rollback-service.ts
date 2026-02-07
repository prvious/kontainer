import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface RollbackServiceResponse {
    status: string;
}

export function useRollbackService(serviceId: string) {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: () =>
            invoke<RollbackServiceResponse>("rollback_service", {
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
