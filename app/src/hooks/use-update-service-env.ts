import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { EnvVar } from "@/types/service-detail";

interface UpdateServiceEnvResponse {
    env: EnvVar[];
}

export function useUpdateServiceEnv(serviceId: string) {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (env: EnvVar[]) =>
            invoke<UpdateServiceEnvResponse>("update_service_env", {
                url: agentUrl,
                token,
                serviceId,
                env,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["service", serviceId] });
            queryClient.invalidateQueries({ queryKey: ["services"] });
        },
    });
}
