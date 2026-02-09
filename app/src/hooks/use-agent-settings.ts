import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { AgentSettings } from "@/types/settings";

export function useAgentSettings() {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["agent-settings"],
        queryFn: () =>
            invoke<AgentSettings>("get_agent_settings", {
                url: agentUrl,
                token,
            }),
        enabled: !!agentUrl && !!token,
        select: (data) => data.settings,
    });
}
