import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface CreateSecretResponse {
    id: string;
    name: string;
}

export function useCreateSecret() {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: ({ name, data }: { name: string; data: string }) =>
            invoke<CreateSecretResponse>("create_secret", {
                url: agentUrl,
                token,
                name,
                data,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["secrets"] });
        },
    });
}
