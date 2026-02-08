import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface CreateConfigResponse {
    id: string;
    name: string;
}

export function useCreateConfig() {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: ({ name, data }: { name: string; data: string }) =>
            invoke<CreateConfigResponse>("create_config", {
                url: agentUrl,
                token,
                name,
                data,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["configs"] });
        },
    });
}
