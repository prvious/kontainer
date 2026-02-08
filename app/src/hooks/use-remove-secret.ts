import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface RemoveSecretResponse {
    removed: boolean;
}

export function useRemoveSecret() {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (secretId: string) =>
            invoke<RemoveSecretResponse>("remove_secret", {
                url: agentUrl,
                token,
                secretId,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["secrets"] });
        },
    });
}
