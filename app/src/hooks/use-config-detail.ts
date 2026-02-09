import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/use-auth";
import type { ConfigDetail } from "@/types/configs";

export function useConfigDetail(configId: string) {
    const { agentUrl, token } = useAuth();

    return useQuery({
        queryKey: ["config", configId],
        queryFn: () => invoke<ConfigDetail>("inspect_config", { url: agentUrl, token, configId }),
        enabled: !!agentUrl && !!token && !!configId,
    });
}
