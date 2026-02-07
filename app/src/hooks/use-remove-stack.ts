import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

interface RemoveStackResponse {
    removed: number;
}

export function useRemoveStack() {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (stackName: string) =>
            invoke<RemoveStackResponse>("remove_stack", {
                url: agentUrl,
                token,
                stackName,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["stacks"] });
            queryClient.invalidateQueries({ queryKey: ["services"] });
        },
    });
}
