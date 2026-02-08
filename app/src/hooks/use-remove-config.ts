import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface RemoveConfigResponse {
    removed: boolean;
}

export function useRemoveConfig() {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (configId: string) =>
            invoke<RemoveConfigResponse>("remove_config", {
                url: agentUrl,
                token,
                configId,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["configs"] });
        },
    });
}
