import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface ScaleServiceResponse {
    replicas: number;
}

export function useScaleService(serviceId: string) {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (replicas: number) =>
            invoke<ScaleServiceResponse>("scale_service", {
                url: agentUrl,
                token,
                serviceId,
                replicas,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["service", serviceId] });
            queryClient.invalidateQueries({ queryKey: ["services"] });
        },
    });
}
