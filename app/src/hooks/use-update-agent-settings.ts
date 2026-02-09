import { useMutation, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";

export function useUpdateAgentSettings() {
    const { agentUrl, token } = useAuth();
    const queryClient = useQueryClient();

    return useMutation({
        mutationFn: (settings: Record<string, string>) =>
            invoke("update_agent_settings", {
                url: agentUrl,
                token,
                settings,
            }),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["agent-settings"] });
        },
    });
}
